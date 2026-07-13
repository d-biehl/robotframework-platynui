import base64
import time
from dataclasses import dataclass, replace
from functools import cached_property
from pathlib import Path
from typing import Any, Literal, TypedDict, cast

from assertionengine import AssertionOperator, verify_assertion
from platynui_native import (
    Activatable,
    AttributeNotFoundError,
    Closeable,
    EvaluatedAttribute,
    EvaluationError,
    KeyboardOverridesLike,
    KeyboardProfileLike,
    Maximizable,
    Minimizable,
    Movable,
    Point,
    PointerButton,
    PointerButtonLike,
    PointerOverridesLike,
    PointerProfileLike,
    PointerSettingsLike,
    Rect,
    RectLike,
    Resizable,
    Restorable,
    Runtime,
    UiNode,
    UiValue,
)
from robot.api import logger
from robot.api.deco import library
from robot.libraries.BuiltIn import BuiltIn
from robot.running.context import EXECUTION_CONTEXTS

from ..__version__ import __version__
from .._assertable import assertable
from .._our_libcore import OurDynamicCore, keyword


class BareMetalError(Exception):
    """Base exception for BareMetal library errors."""


class ElementNotFoundError(BareMetalError):
    """Raised when a UiNode cannot be found for a given query within the specified timeout."""


class ResultTypeError(BareMetalError):
    """Raised when a query result cannot be interpreted as the expected type (e.g., UiNode)."""


class NoQueryError(BareMetalError):
    """Raised when a UiNodeDescriptor is called without a query to resolve the node."""


class InvalidSelectorError(BareMetalError):
    """Raised when a selector passed to Set Root cannot be parsed as a valid XPath expression."""


class ElementStillPresentError(BareMetalError):
    """Raised when an element is still present/valid after a `Wait Until Gone` timeout elapses."""


PLATYNUI_QUERY_SETTINGS = (
    'PLATYNUI_QUERY_SETTINGS'  # Variable name for storing the current query settings in Robot Framework variables
)


@dataclass(frozen=True, slots=True, kw_only=True)
class QuerySettings:
    """Fully resolved settings for the query wait/retry loop.

    These are the values ``UiNodeDescriptor.__call__`` consults while waiting for a node to appear:
    how long to keep retrying (``timeout``), how long to pause between attempts (``retry_interval``)
    and whether to swallow evaluation errors instead of raising them (``ignore_exceptions``).
    """

    timeout: float = 30.0
    retry_interval: float = 0.1
    ignore_exceptions: bool = False


class QuerySettingsDict(TypedDict, total=False):
    """Partial override of `QuerySettings`; any omitted key inherits the layer below.

    This is the shape callers pass — at import (``query_settings=``), per keyword call
    (``query_overrides=``) or through `Set Query Settings` — as a plain dict, e.g. ``{'timeout': 10}``.
    """

    timeout: float
    retry_interval: float
    ignore_exceptions: bool


class UiNodeDescriptor:
    """Descriptor wrapper allowing lazy resolution of a UiNode from a query string.

    The descriptor either holds a concrete UiNode or an expression string that will be
    evaluated using the associated BareMetal library instance when called.
    """

    def __init__(
        self,
        node: UiNode | None,
        query: str | None,
        library: 'BareMetal',
        parent: 'UiNodeDescriptor | None' = None,
        is_root_binding: bool = False,
    ) -> None:
        self.node = node
        self.query = query
        self.library = library
        # When this descriptor is used as a stored root, ``parent`` is the root that was effective
        # when it was set; a relative query then drills into it. ``is_root_binding`` marks a
        # descriptor created by ``set_root`` so that restoring a saved root keeps its chain as-is
        # instead of being re-parented.
        self.parent = parent
        self.is_root_binding = is_root_binding
        # Per-call query-settings override (partial), set from each wait keyword's query_overrides
        # argument before it resolves this descriptor — unconditionally, so a stale value never leaks
        # via the shared descriptor cache. Resolved against the scoped/default settings in __call__.
        self.overrides: QuerySettingsDict | None = None

    def __call__(self, no_root: bool = False) -> UiNode:
        if isinstance(self.node, UiNode):
            if self.node.is_valid():
                return self.node

        if self.query is None:
            raise NoQueryError('UiNodeDescriptor has no query to resolve the node')

        # Resolving a stored root (no_root=True) evaluates the query against the captured parent
        # chain, so a relative root drills into the enclosing root while an absolute query ignores
        # it. As a query target (no_root=False) it evaluates against the library's current root.
        context = (self.parent(True) if self.parent is not None else None) if no_root else self.library.root

        # Effective settings for this resolution: the scoped/default base, with this call's partial
        # override applied on top. Computed once — neither layer changes during a synchronous resolve.
        base = self.library.query_settings
        settings = replace(base, **self.overrides) if self.overrides else base

        start_time = time.monotonic()
        result: UiNode | UiValue | EvaluatedAttribute | None = None
        while True:
            try:
                result = self.library.runtime.evaluate_single(self.query, context)
            except (SystemExit, KeyboardInterrupt):
                raise  # Don't interfere with user-initiated interrupts
            except Exception as e:
                if not settings.ignore_exceptions:
                    raise e
                result = None  # Swallow the error, but keep honouring the timeout below
            else:
                if result is not None:
                    break

            # Not resolved yet — no match, or a swallowed error. Retry until the timeout elapses, so
            # ignore_exceptions cannot spin forever on a persistently failing query.
            if (time.monotonic() - start_time) > settings.timeout:
                raise ElementNotFoundError(
                    f'No UiNode found for UiNodeDescriptor query {self.query!r} within '
                    f'timeout of {settings.timeout} seconds.'
                )

            time.sleep(settings.retry_interval)
            self.library.runtime.clear_cache()  # Clear runtime cache to attempt to resolve transient UI states

        if not isinstance(result, UiNode):
            raise ResultTypeError(f'Query for UiNodeDescriptor {self.query!r} did not return a UiNode, got: {result!r}')

        self.node = result  # Cache resolved node

        return result

    @staticmethod
    def convert(value: str | UiNode, library: 'BareMetal') -> 'UiNodeDescriptor':
        if isinstance(value, UiNode):
            return UiNodeDescriptor(value, None, library)
        return library.descriptor_from_query(value)


PLATYNUI_ROOT_DESCRIPTOR = (
    'PLATYNUI_ROOT_DESCRIPTOR'  # Variable name for storing the current root descriptor in Robot Framework variables
)


def _assertion_value(result: Any) -> Any:
    """Unwrap an evaluated query result to the value an assertion should apply to.

    ``evaluate_single`` for a ``.../@X`` step returns an `EvaluatedAttribute`; assertions
    must run against its typed ``.value`` so that the AssertionEngine operators which need
    the native value — ``contains``, ``matches`` and the order operators — work. ``UiNode``
    and native values pass through unchanged.
    """
    if isinstance(result, EvaluatedAttribute):
        return result.value
    return result


ScrollDirection = Literal['UP', 'DOWN', 'LEFT', 'RIGHT']
"""The visually intended direction for `Pointer Scroll`."""

# One mouse-wheel notch in scroll-delta units. Win32's ``WHEEL_DELTA`` and Wayland's
# ``axis_value120`` both use 120 units per notch, so it is the cross-platform wheel step.
_WHEEL_DELTA = 120.0

# Per-direction unit sign for the native ``(horizontal, vertical)`` scroll delta. A *negative*
# component scrolls the content the visually intended way on each axis, so both ``DOWN`` and
# ``RIGHT`` are negative: vertical ``DOWN`` is the platform's natural negative step (``scroll_step``
# defaults to ``(0, -120)``; X11 wheel button 5), and horizontal ``RIGHT`` matches it (X11 wheel
# button 7). The keyword owns this sign so callers never write signed deltas; flip a row here if a
# platform ever disagrees (the egui acceptance suite asserts the visible direction).
_SCROLL_AXIS_SIGN: dict[ScrollDirection, tuple[float, float]] = {
    'UP': (0.0, 1.0),
    'DOWN': (0.0, -1.0),
    'LEFT': (1.0, 0.0),
    'RIGHT': (-1.0, 0.0),
}


def _scroll_delta(direction: ScrollDirection, ticks: int) -> tuple[float, float]:
    """Map a `Pointer Scroll` ``direction`` and notch count to a native ``(horizontal, vertical)`` delta.

    One notch is `_WHEEL_DELTA` units. The keyword owns the sign so the requested direction is the
    one that becomes visible, freeing callers from the signed-delta convention.
    """
    signs = _SCROLL_AXIS_SIGN.get(direction)
    if signs is None:
        raise ValueError(f'Invalid scroll direction {direction!r}; expected one of UP, DOWN, LEFT, RIGHT')
    magnitude = _WHEEL_DELTA * ticks
    return (signs[0] * magnitude, signs[1] * magnitude)


@library(
    scope='SUITE',
    version=__version__,
    converters={UiNodeDescriptor: UiNodeDescriptor.convert},
    doc_format='ROBOT',
)
class BareMetal(OurDynamicCore):
    """Robot Framework library for automating native desktop applications with PlatynUI.

    BareMetal drives real applications through the operating system's accessibility services — UI
    Automation on Windows, AT-SPI2 on Linux — and presents the whole desktop as one tree of UI
    elements you query with XPath, the same way on every platform. You address an element by what it
    is, not where it is, so a selector keeps matching when a window moves or is resized.

    == Table of contents ==

    %TOC%

    = Elements and attributes =

    Everything you automate is an *element* — a window, a button, a list, a single row in that list,
    a text box. PlatynUI presents the whole desktop as one tree of these elements: each running
    application is a branch, and the elements inside it are the branches below. The tree is live: it
    always reflects what is on screen at that moment, so as an application opens a window, loads
    content or closes a dialog, elements appear in and vanish from the tree with it. Every query is
    answered against this current state, never a stored snapshot.

    Every element has a *role* and a set of *attributes*.

    An element's *role* is the kind of thing it is — ``Button``, ``Window``, ``List``, ``ListItem``
    — and it is the name you write for the element in a path. The role is what makes a selector
    portable: a confirmation button is a ``Button`` whether it sits in a toolbar or a dialog, on
    Windows or on Linux, so the same path keeps matching it across applications and platforms.

    An element's *attributes* are the facts about it — what it is called, whether it can be used,
    where it sits on screen. You use them two ways: you *filter* on an attribute to single out the
    element you mean (two buttons that differ only by their ``@Name``), and you *read* an attribute
    to check the application's state (is this checkbox enabled, what does that label say). An
    attribute is always written with a leading ``@``. The ones you reach for most often:

    | = Attribute = | = Meaning = |
    | ``@Name`` | the visible label or caption |
    | ``@Id`` | a stable, language-independent identifier — prefer it when it is set |
    | ``@Role`` | the element's kind, the same name you use for it in a path |
    | ``@Bounds`` | the element's screen rectangle |
    | ``@IsVisible`` | whether the element is currently shown |
    | ``@IsEnabled`` | whether the element can be interacted with |
    | ``@IsFocused`` | whether the element currently holds the keyboard focus |

    These few are *standard*: PlatynUI gives them the same name on every platform, so a path that
    filters on ``@Name`` or ``@IsEnabled`` behaves the same on Windows and on Linux. They are not
    the whole story, though. An element also carries whatever else fits it — a slider its value, a
    selectable row whether it is selected, a window whether it is ``@IsActive``, ``@IsMinimized`` or
    ``@IsMaximized``, an application its process details. And beneath the standard set it can surface
    raw, technology-specific values straight from the accessibility platform under the ``native:``
    prefix; those are passed through untouched and differ from one platform to the next. Which
    attributes a target actually offers therefore depends on the application and the platform
    underneath it; the PlatynUI Inspector shows exactly what a given element exposes and is the
    source of truth.

    == Namespaces ==

    A role belongs to one of four namespaces, written as a prefix. ``control:`` is the default for
    element names — ``//Button`` is ``//control:Button`` — so only the others are spelled out:

    | = Prefix = | = Selects = |
    | ``control:`` | ordinary elements — the default |
    | ``item:`` | items inside containers: ``ListItem``, ``MenuItem``, ``TabItem``, ``TableCell`` ... |
    | ``app:`` | a running application (see `Targeting a specific application`) |
    | ``native:`` | raw, technology-specific roles and attributes |

    Attributes have no default namespace (unlike element names), so a standard attribute is written
    bare — ``@Name``, ``@Id``, ``@Bounds``, an application's ``@ProcessId`` — while a
    technology-specific one keeps its prefix, chiefly the raw values surfaced under ``@native:...``.

    | @{rows}=    `Query`    Window[@Name="Mail"]//List[@Name="Inbox"]/item:ListItem

    = Finding elements =

    `Query` evaluates an XPath 2.0 expression against the live tree. You select an element by its
    role and filter on its attributes in ``[...]`` — exactly, partially, by regular expression, or by
    position:

    | `Pointer Click`    //Button[@Name="OK"]    # exact text
    | `Keyboard Type`    //Edit[starts-with(@Name, "Address")]    12 Main St    # partial text
    | `Get Attribute`    //CheckBox[matches(@Name, "Option [0-9]+")]    IsEnabled    # regular expression
    | `Pointer Click`    (//Button)[last()]    # by position

    ``=`` is exact and case-sensitive; use ``contains()``, ``starts-with()`` or ``matches()`` for
    partial text. The usual XPath 2.0 string, numeric and boolean functions are available. Filter on
    ``@Id`` instead of ``@Name`` when it is set — it is stable and language-independent.

    To match on more than one fact, combine conditions with ``and`` / ``or`` in one predicate, or
    chain predicates. A boolean attribute such as ``@IsVisible`` is compared with ``true()`` or
    ``false()`` — not the strings ``"true"``/``"false"``:

    | `Pointer Click`    //Button[@Name="Save" and @IsVisible=true()]    # named *and* visible
    | `Pointer Click`    //Button[@Name="OK" or @Name="Yes"]    # either label

    == Narrowing the search ==

    A bare ``//`` is quick to write, but it searches the *whole desktop*: ``//Edit[@Name="Street"]``
    matches every street field on screen, across every window of every program. And the same label
    often repeats *within* one window — a customer form may carry the same *Street* field in both its
    *Delivery* and *Billing* address groups — so even a single window can hold several matches. You
    narrow the path until it names exactly the field you mean, and each step you add is more precise
    *and* faster, because the search then stays inside a smaller part of the tree.

    First, *to its window*. A window is a child of the desktop, and the desktop is the default context
    (see `Scoping queries to a container`), so a plain ``Window`` step reaches a top-level window — no
    leading ``/`` — and ``//`` descends into it. Being relative, the step also follows any `Set Root`
    you have set. (Avoid a leading ``//Window``: it rescans the whole desktop and would also match a
    window nested inside another.)

    | `Keyboard Type`    Window[@Name="Customer"]//Edit[@Name="Street"]    221B Baker St

    Here the window alone is still not enough — the *Street* field lives in two groups, so the path
    matches both. Narrow on *to the group* that surrounds the field you want:

    | `Keyboard Type`    Window[@Name="Customer"]//Group[@Name="Billing"]//Edit[@Name="Street"]    221B Baker St

    Finally, when several programs — or several copies of one — are open, narrow *to the application*:
    match its ``app:`` node by name so the query cannot stray into another program, or by ``ProcessId``
    to single out one exact instance (see `Targeting a specific application`). You can spell the whole
    path out in one go:

    | `Focus`    app:Application[@Name="CRM"]/Window[@Name="Customer"]//Group[@Name="Billing"]//Edit[@Name="Street"]

    That prefix is a mouthful to repeat on every step, so this is usually the point to set it once as
    the root with `Set Root` and address the rest relative to it (see `Scoping queries to a
    container`):

    | `Set Root`    /app:Application[@Name="CRM"]
    | `Focus`       Window[@Name="Customer"]//Group[@Name="Billing"]//Edit[@Name="Street"]

    == Steps and axes ==

    A path is a series of *steps* separated by ``/``, and each step moves from where you are now to
    elements in one direction — that direction is its *axis*. The forms you have already seen are
    the everyday ones, written in shorthand:

    | = Step = | = Moves to = |
    | ``/`` | the direct children, one level down |
    | ``//`` | any descendant, at any depth below |
    | ``.`` | the current element itself |
    | ``..`` | the parent |
    | ``@`` | an attribute of the current element (a value, not another element) |
    | ``*`` | any element, whatever its role |

    A leading ``/`` or ``//`` starts from the desktop, so the path is *absolute*; a path without one —
    a plain ``Window`` step, or a leading ``.`` or ``..`` — starts from the current root, so it is
    *relative*. This is the distinction `Scoping queries to a container` builds on.

    When you need a direction the shorthand has no symbol for — an ancestor, a sibling — name the
    axis in full as ``axis::Role``:

    | ${dialog}=    `Query`    //Button[@Name="OK"]/ancestor::Dialog    only_first=${True}
    | @{later}=     `Query`    //item:ListItem[@Name="Today"]/following-sibling::item:ListItem

    The named axes are ``child::``, ``descendant::``, ``parent::``, ``ancestor::``,
    ``ancestor-or-self::``, ``following-sibling::``, ``preceding-sibling::`` and ``following::``.

    = Acting on elements =

    The action keywords — pointer (click, press, move), keyboard, focus, window control (move,
    resize, minimize, maximize, activate, close), screenshots and highlight — take either a selector
    or an element you captured with `Query`. Each waits for its target (see `Waiting for elements`):

    | `Maximize Window`   Window[@Name="Editor"]
    | `Pointer Click`     Window[@Name="Editor"]//Button[@Name="New"]
    | `Keyboard Type`     Window[@Name="Editor"]//Edit[@Name="Title"]    Q3 Report

    Or capture an element once with `Query` and reuse it — every keyword takes it the same way:

    | ${save}=    `Query`    Window[@Name="Editor"]//Button[@Name="Save"]    only_first=${True}
    | `Pointer Click`    ${save}
    | `Highlight`        ${save}

    == Bringing windows to the front ==

    Acting on a window that sits in the background is unreliable, so most action keywords raise the
    target's top-level window to the front before they act. The reason differs by input kind but
    points the same way: a pointer click lands on whatever window is visually on top at its screen
    coordinates, and keyboard input goes to the window the desktop currently treats as active —
    focusing an element on its own only moves focus *within* its application and does not guarantee
    that its window is the active one. `Take Screenshot` of an element and `Highlight` are in the
    same boat: they read the element's on-screen rectangle, which a window in front would cover.

    So the pointer keywords, the keyboard keywords, `Focus`, `Take Screenshot` (for an element) and
    `Highlight` bring the element's top-level window forward, then act. This is governed by the
    ``auto_activate`` import setting (default ``${True}``); turn it off to leave the window stacking
    order untouched — for instance when you deliberately drive a background window, or do not want a
    test to steal the foreground. Each of these keywords also takes a per-call ``activate`` that
    overrides the import default for a single call:

    | `Pointer Click`    Window[@Name="Settings"]//Button[@Name="OK"]    activate=${False}

    The window keywords (`Move Window`, `Maximize Window`, `Close Window`) are the exception: they
    drive a window through its own controls rather than its screen position (see `Window control`),
    so they have no ``activate`` and never need it. `Activate Window` and `Bring To Front` do raise a
    window — but that is the whole point of those two.

    == Window control ==

    The window keywords — `Move Window`, `Resize Window`, `Minimize Window`, `Maximize Window`,
    `Restore Window`, `Activate Window` and `Close Window` — differ from the rest in one way: they do
    not merely *find* a window, they ask it to change state. Whether a particular window can be
    maximized, moved or closed is not assumed — it is a capability the application and the window
    manager expose, and not every window offers every one (a fixed-size tool window may refuse to
    resize; a splash screen may have no close affordance). Each keyword probes for the capability it
    needs and fails with a clear error when the target does not offer it, rather than appearing to
    succeed while nothing happens.

    Two of them only change which window is in front: `Activate Window` (like `Bring To Front`) raises
    a window and gives it the keyboard focus. The others change a window's display state — and
    `Restore Window` is the inverse of `Minimize Window` and `Maximize Window`, returning a minimized
    or maximized window to the floating size and position it had before.

    = What a query gives you =

    `Query` returns a list: empty if nothing matched, one entry per match. Pass ``only_first=${True}``
    for just the first match, or ``${None}`` when there is none — the way to check that something is
    absent. The expression's last step decides what each entry is:

    - an *element*, when it selects nodes (``//Button``) — a handle for the action keywords, exposing
      ``${el.role}``, ``${el.name}``, ``${el.id}`` and ``${el.runtime_id}``;
    - an *attribute*, when it ends in an attribute step (``.../@Name``) — ``${attr.value}`` holds the
      typed value (``@IsEnabled`` a boolean, ``@Bounds`` a ``Rect``) and ``${attr.owner()}`` the
      element it belongs to;
    - a plain *value*, when it computes one — ``count(//Button)`` or ``string-join(//Button/@Name, ", ")``.

    | @{buttons}=    `Query`    //Button    # elements
    | @{names}=      `Query`    //Button/@Name    # attributes
    | ${n}=          `Query`    count(//Button)    only_first=${True}    # a value

    The attribute step is just another step, so two more things follow. Walk an axis before it and
    you read from a *related* element rather than the matched one — the name of the window a button
    sits in is its ``ancestor::Window/@Name``. And since ``//Button/@Name`` above already yields one
    name per button, ``string-join(...)`` folds them into a single string:

    | ${window}=    `Query`    //Button[@Name="OK"]/ancestor::Window/@Name    only_first=${True}    # its window's name
    | ${names}=     `Query`    string-join(//Button/@Name, ", ")    only_first=${True}    # "OK, Cancel, ..."

    = Reading and checking values =

    `Get Attribute` reads one attribute of one element and, in the same call, can assert on it: add
    an operator and the expected value, and the keyword fails if it does not hold.

    | ${enabled}=    `Get Attribute`    Window[@Name="Settings"]//Button[@Name="Save"]    IsEnabled
    | `Get Attribute`    Window[@Name="Settings"]//Button[@Name="Save"]    IsEnabled    ==    ${True}
    | `Get Attribute`    Window[@Name="Settings"]//CheckBox[@Name="Dark mode"]    IsEnabled    ==    ${False}

    Operators include ``==``, ``!=``, ``contains``, ``starts``, ``ends`` and ``matches`` (see
    [https://github.com/MarketSquare/AssertionEngine|AssertionEngine]). For several values at once,
    or a computed one, read them through `Query` instead.

    = Scoping queries to a container =

    Every query runs against a *context node*, the desktop by default. Scope it to a container you
    found and relative selectors (a plain ``Window`` step, ``.//``, ``./``, axes like ``child::``)
    search inside it; an absolute ``/`` or ``//`` ignores the context and starts again at the desktop.

    For one query, pass ``root``:

    | ${inbox}=    `Query`    Window[@Name="Mail"]//List[@Name="Inbox"]    only_first=${True}
    | @{items}=    `Query`    .//item:ListItem    root=${inbox}

    `Set Root` makes a root the default for everything that follows, so you stop repeating a long
    prefix. A new `Set Root` is itself resolved against the current one: a relative root narrows it
    step by step (``..`` widens again), an absolute root switches away. It returns the previous root,
    and ``Set Root    ${None}`` clears it back to the desktop:

    | `Set Root`        /app:Application[@Name="Editor"]    # work inside the Editor from here on
    | `Set Root`        .//Dialog[@Name="Save"]              # narrow to its Save dialog
    | `Pointer Click`   .//Button[@Name="Save"]

    == Scope ==

    A root lives as long as the Robot Framework variable it is kept in; ``scope`` chooses which, and
    each scope clears itself when it ends — no teardown to remember:

    | = scope = | = The root applies to = |
    | ``LOCAL`` (default) | the current test or keyword, not the keywords it calls |
    | ``TEST`` | the whole test, including called keywords |
    | ``SUITE`` | every test in the suite |

    | `Set Root`    /app:Application[@Name="Editor"]    scope=SUITE

    The root stores the *query*, not a fixed node, so it re-resolves against the live tree and keeps
    working even after its window closes and reopens. (It lives in ``${PLATYNUI_ROOT_DESCRIPTOR}``,
    but set it through `Set Root` — the raw variable holds an internal descriptor.)

    = Targeting a specific application =

    When several programs are open, anchor your selectors to the program itself so they cannot drift
    into the wrong one. Each application is an *application element* in the ``app:`` namespace, with
    the windows and elements it owns beneath it, so matching its name scopes everything below it to
    that program. Either prefix a single query with it:

    | `Pointer Click`    app:Application[@Name="Editor"]//Button[@Name="Save"]

    or make it the root, so every relative selector that follows stays inside that program:

    | `Set Root`        /app:Application[@Name="Editor"]
    | `Pointer Click`   .//Button[@Name="Save"]

    The node also carries the process. If you launched the program and know its process id, matching
    on it pins the query to that one instance even when several copies run (``ProcessId`` is a number
    — no prefix, no quotes; ``Start Process`` and ``Get Process Id`` are Robot Framework's built-in
    Process keywords):

    | ${proc}=    Start Process     editor
    | ${pid}=     Get Process Id    ${proc}
    | `Set Root`    /app:Application[@ProcessId=${pid}]

    = Waiting for elements =

    Action and read keywords *wait* for their target, and knowing why explains most of what follows.
    Real applications are asynchronous: after you click *New*, the editor window needs a moment to
    open; a list fills in only once its data has loaded; a dialog you dismiss lingers for a few frames
    before it is gone. If a keyword inspected the tree only once, your tests would pass or fail at the
    mercy of how fast the machine happens to be that day.

    So every action and read keyword re-evaluates your selector against the live tree every
    ``retry_interval`` and acts the moment the element appears; only once ``timeout`` has elapsed does
    it give up and fail, with an error naming the selector and how long it waited. This is why you
    almost never need an explicit `Sleep`: state *what* you expect to be there and the keyword waits
    for exactly that, no longer.

    `Query` is the deliberate exception. It is a snapshot of the tree at the instant you call it and
    returns straight away with whatever matches right then — possibly nothing. That makes it the tool
    for *asking about* the UI rather than *acting on* it: counting how many rows a table has, or
    confirming a dialog is *gone* — a waiting keyword would block for the full timeout before it could
    ever tell you that something is absent.

    Three settings govern the wait, together the *query settings*:

    | = Setting = | = Meaning = |
    | ``timeout`` | how long to keep retrying before giving up, in seconds (default ``30``) |
    | ``retry_interval`` | the pause between attempts, in seconds (default ``0.1``) |
    | ``ignore_exceptions`` | keep retrying instead of failing when an attempt raises (default ``False``) |

    ``timeout`` is the headroom for the slowest thing you wait on. Thirty seconds suits most UIs; raise
    it when an application is slow to start or an action takes a while to settle (a save that talks to
    the network), and lower it when you *expect* an element to be present already and would rather fail
    fast than sit through the default.

    ``retry_interval`` is how long the lookup pauses between attempts. The default tenth of a second is
    responsive without hammering the platform's accessibility layer on every poll; you rarely need to
    change it — raise it only if polling itself turns out to be expensive on a particular provider.

    ``ignore_exceptions`` concerns the attempts that do not merely *miss* but *raise*. While an
    application rebuilds part of its interface, a provider can momentarily throw — a node disappears in
    the middle of a traversal, or the accessibility bridge returns a transient error. With this off
    (the default) the first such error fails the keyword at once; with it on, the loop swallows the
    error and keeps retrying until the timeout, so a target whose container is being torn down and
    rebuilt still resolves once things settle. Turn it on deliberately and narrowly: it also masks a
    genuinely wrong selector that would otherwise fail fast, turning a clear error into a long wait.

    == Tuning the wait ==

    You set the query settings on three levels, each with a different reach, naming only the fields you
    want to change — the rest are inherited.

    *For the whole suite*, as the baseline, when you import the library:

    | Library    PlatynUI.BareMetal    query_settings={'timeout': 60}

    *For a stretch of a test* that needs more (or less) patience than the rest — a slow login, say —
    with `Set Query Settings`. It works exactly like `Set Root`: the change lives as long as its
    ``LOCAL``/``TEST``/``SUITE`` scope and clears itself when that scope ends, so there is no teardown
    to remember.

    | `Set Query Settings`    {'timeout': 60}    scope=TEST

    *For one stubborn element*, with a keyword's ``query_overrides`` argument, which affects that call
    alone:

    | `Pointer Click`    Window[@Name="Editor"]//Button[@Name="Save"]    query_overrides={'timeout': 10}

    The three form a chain, and the most specific wins: a per-call ``query_overrides`` overrides the
    scope, which overrides the import baseline. The import and scope levels apply to *every* lookup —
    including the re-resolution of a `Set Root` root — whereas ``query_overrides`` tunes only that one
    keyword's own target; if you need to wait longer for the root as well, set it at the scope level.

    One subtlety worth keeping in mind: ``timeout`` bounds *each* element resolution, not the keyword
    as a whole. A keyword that resolves both a root and a target performs two lookups, each allowed its
    own ``timeout`` — so in the worst case a single keyword can wait longer than the number you set.

    == Waiting explicitly ==

    Most of the time you never wait by hand — you state what you expect and the action or read keyword
    waits for exactly that. Now and then, though, you want a *pure* synchronization point: wait for a
    splash screen to vanish before carrying on, wait for a window to open without acting on it yet, or
    wait for a row count to settle. Three keywords cover this, all governed by the same query settings as
    everything else (see `Tuning the wait`) and tuned per call with ``query_overrides``:

    - `Wait Until Exists` waits for a selector to resolve to an element and *returns* it — the waiting
      companion to `Query`, which does not wait. It is element-only.
    - `Wait Until Gone` waits for the opposite: a selector that matches nothing, or a captured element
      that becomes invalid. For a value condition — a count dropping to zero, say — use `Wait Until
      Query` instead.
    - `Wait Until Query` waits until an XPath result satisfies a condition. It takes the same assertion
      operators as `Get Attribute`; with no operator it waits until the result is truthy. Unlike `Get
      Attribute` it works on the raw query result, so an attribute step yields the attribute value and
      ``count(...)`` a number.

    | `Wait Until Exists`    Window[@Name="Save As"]    # wait for the dialog, then return it
    | `Wait Until Gone`      Window[@Name="Please wait"]    # wait until the splash is gone
    | `Wait Until Query`     count(//item:ListItem)    >    0    # wait until the list has filled

    = Input timing and motion =

    BareMetal generates real keystrokes and pointer movements. Out of the box they are quick and
    direct, which suits most automation — but you can slow them down or shape them when an
    application needs a moment to settle between inputs, when you want human-like pointer movement,
    or when a screen recording should be easy to follow.

    There are two places to tune this. ``keyboard_profile``, ``pointer_settings`` and
    ``pointer_profile`` set the defaults for the whole suite when you import the library; every
    keyboard and pointer keyword also takes an ``overrides`` argument that adjusts a single call.
    Each is a dict, you name only the fields you want to change, and a per-call ``overrides`` wins
    over the profile for that one call.

    == Keyboard timing ==

    ``keyboard_profile`` and the keyboard keywords' ``overrides`` share these millisecond delays:

    | = Field = | = Delay = |
    | ``press_delay_ms`` | after a key is pressed, before it is released |
    | ``release_delay_ms`` | after a key is released |
    | ``between_keys_delay_ms`` | between one key and the next |
    | ``chord_press_delay_ms`` | between presses within a modifier chord |
    | ``chord_release_delay_ms`` | between releases within a chord |
    | ``after_sequence_delay_ms`` | after the whole key sequence |
    | ``after_text_delay_ms`` | after typed text |

    | Library    PlatynUI.BareMetal    keyboard_profile={'press_delay_ms': 20, 'between_keys_delay_ms': 30}
    | `Keyboard Type`    Window[@Name="Login"]//Edit[@Name="PIN"]    1234    overrides={'between_keys_delay_ms': 200}

    == Pointer clicks ==

    ``pointer_settings`` holds the click semantics:

    | = Field = | = Meaning = |
    | ``double_click_time_ms`` | how quickly two clicks count as a double-click |
    | ``double_click_size`` | how close together the two clicks must land |
    | ``default_button`` | the button a click uses when none is given |

    | Library    PlatynUI.BareMetal    pointer_settings={'double_click_time_ms': 400}

    == Pointer motion ==

    ``pointer_profile`` and the pointer keywords' ``overrides`` shape how the pointer travels.
    ``motion`` picks the path:

    | = Mode = | = Path to the target = |
    | ``DIRECT`` | jump straight there, no visible travel |
    | ``LINEAR`` | a straight line |
    | ``BEZIER`` | a curved line |
    | ``OVERSHOOT`` | overshoot, then settle back |
    | ``JITTER`` | a straight line with a small wobble |

    These knobs refine the movement and the click pacing:

    | = Field = | = Effect = |
    | ``speed_factor`` | scales the overall movement speed |
    | ``overshoot_ratio`` | how far an ``OVERSHOOT`` goes past the target |
    | ``curve_amplitude`` | how pronounced the ``BEZIER`` curve is |
    | ``jitter_amplitude`` | the size of the ``JITTER`` wobble |
    | ``multi_click_delay_ms`` | the pause between clicks of a multi-click |

    | Library    PlatynUI.BareMetal    pointer_profile={'motion': 'BEZIER', 'speed_factor': 0.5}
    | `Pointer Click`    Window[@Name="Editor"]//Button[@Name="Save"]    overrides={'speed_factor': 0.3}

    == Pointer scrolling ==

    `Pointer Scroll` turns the mouse wheel by a number of notches (``ticks``) in a ``direction`` —
    ``UP``, ``DOWN``, ``LEFT`` or ``RIGHT`` — over an element, at coordinates, or at the current
    pointer position; you never write a signed delta. Its pacing lives in ``pointer_profile`` (and a
    per-call ``overrides``):

    | = Field = | = Effect = |
    | ``scroll_step`` | the wheel delta applied per animation step |
    | ``scroll_delay_ms`` | the pause between scroll steps |

    | `Pointer Scroll`    Window[@Name="Mail"]//List[@Name="Inbox"]    direction=DOWN    ticks=${3}

    = A short example =

    | `Pointer Click`     Window[@Name="Editor"]//Button[@Name="New"]    # start a new document
    | `Keyboard Type`     ${None}    Q3 Report    # type into the focused field
    | `Get Attribute`     Window[@Name="Editor"]//Button[@Name="Save"]    IsEnabled    ==    ${True}    # Save enabled
    | `Take Screenshot`
    """

    def __init__(
        self,
        *,
        keyboard_profile: KeyboardProfileLike | None = None,
        pointer_settings: PointerSettingsLike | None = None,
        pointer_profile: PointerProfileLike | None = None,
        auto_activate: bool = True,
        query_settings: QuerySettingsDict | None = None,
        config: dict[str, Any] | None = None,
        use_mock: bool = False,
    ) -> None:
        """Import the library, optionally tuning window activation and input behaviour.

        | Library    PlatynUI.BareMetal

        All arguments are optional. ``auto_activate`` governs window raising before actions (see
        `Bringing windows to the front`); ``query_settings`` sets how long lookups wait (see
        `Waiting for elements`); the three *profile* arguments set the default timing and motion of
        generated input, each given as a dict of the fields you want to change (see `Input timing
        and motion`):

        | = Argument = | = What it does = |
        | ``auto_activate`` | raise an element's window to the front before acting on it — default ``True`` |
        | ``query_settings`` | query *waiting*: ``timeout``, ``retry_interval`` and ``ignore_exceptions`` |
        | ``keyboard_profile`` | keyboard *timing*: delays around key presses, chords and whole sequences |
        | ``pointer_settings`` | pointer *behaviour*: double-click interval and size, and the default button |
        | ``pointer_profile`` | pointer *motion*: speed, acceleration, curve, overshoot and jitter |
        | ``config`` | *session* binding: which display server and accessibility bus this runtime drives |

        | Library    PlatynUI.BareMetal    auto_activate=${False}    query_settings={'timeout': 60}

        ``config`` binds this runtime to a specific session and is separate from the behavioural
        dictionaries above: where those tune *how* input and waiting behave, ``config`` selects *what*
        the runtime connects to. It is a nested dict with two buckets, each keyed by a
        backend/provider id — ``platform`` (the display server) and ``providers`` (the accessibility
        technology). For example, to pin the X11 display a runtime drives:

        | Library    PlatynUI.BareMetal    config={'platform': {'x11': {'display': ':1'}}}

        The ``platform`` bucket also accepts a reserved ``backend`` selector (``'x11'``, ``'wayland'``,
        ``'windows'`` …) that forces a backend instead of auto-detecting it from the environment, and
        the ``providers`` bucket carries per-provider settings — chiefly ``providers.atspi.bus_address``
        to bind to a chosen session's AT-SPI bus. One dict is portable across operating systems: a
        backend simply ignores the ids and keys it does not recognise, so you may carry every
        platform's block in the same dict. An absent or empty ``config`` reproduces the default
        behaviour — the platform is auto-detected and the accessibility bus is discovered from the
        environment. ``config`` is fixed at construction time (a live connection cannot be re-pointed
        at another display), so there is no per-call override.

        ``use_mock`` exists only for PlatynUI's own development and test suites — it drives a built-in
        stand-in tree instead of the real desktop, selecting the in-memory mock backend regardless of
        ``config``. Leave it at its default; it is not meant for automating applications.
        """
        super().__init__([])
        self._screenshot_counter = 1
        self.use_mock = use_mock
        # Construction-time session binding (immutable); consumed once in _create_runtime().
        self._config = config
        self.auto_activate = auto_activate
        # Library-import defaults — the fallback when no scoped ${PLATYNUI_QUERY_SETTINGS} is set.
        self._default_query_settings = replace(QuerySettings(), **query_settings) if query_settings else QuerySettings()
        self._keyboard_profile = keyboard_profile
        self._pointer_settings = pointer_settings
        self._pointer_profile = pointer_profile
        self._descriptor_cache: dict[str, UiNodeDescriptor] = {}

    def descriptor_from_query(self, query: str) -> UiNodeDescriptor:
        descriptor = self._descriptor_cache.get(query)
        if descriptor is not None:
            return descriptor

        descriptor = UiNodeDescriptor(None, query, self)
        self._descriptor_cache[query] = descriptor
        return descriptor

    def _create_runtime(self) -> Runtime:
        """Create and return the PlatynUI runtime instance.

        This method is called lazily when the `runtime` property is accessed for the first time.
        It allows for deferred initialization of the runtime, which can be beneficial for
        performance and resource management, especially if the library is imported but not
        immediately used.

        Returns:
            Runtime: An instance of the PlatynUI runtime, either a real one or a mock based on configuration.
        """
        # ``use_mock`` is sugar for the in-memory mock backend: it routes through the mock-only
        # provider+platform path, ignoring ``config`` (the mock has no real session to bind to).
        if self.use_mock:
            return Runtime.new_with_mock()

        # Real path: bind to the session named by ``config`` (None ⇒ default, environment-driven).
        return Runtime(self._config)

    @cached_property
    def runtime(self) -> Runtime:
        """Return the PlatynUI BareMetal runtime instance.

        The runtime bridges this Robot Framework library with the native PlatynUI engine,
        enabling XPath 2.0 queries and actions against the UI tree.
        """
        runtime = self._create_runtime()

        if self._keyboard_profile is not None:
            getattr(runtime, 'set_keyboard_profile')(self._keyboard_profile)
        if self._pointer_settings is not None:
            runtime.set_pointer_settings(self._pointer_settings)
        if self._pointer_profile is not None:
            runtime.set_pointer_profile(self._pointer_profile)
        return runtime

    @cached_property
    def _screenshot_path(self) -> Path:
        return Path(str(BuiltIn().get_variable_value('${OUTPUT DIR}', default='screenshots')))

    @property
    def root(self) -> UiNode | None:
        """Return the root UiNode of the current UI tree.

        This is the default context for queries when no root is specified.
        """
        if PLATYNUI_ROOT_DESCRIPTOR in EXECUTION_CONTEXTS.current.variables:  # pyright: ignore[reportOptionalMemberAccess]
            r = EXECUTION_CONTEXTS.current.variables[f'${{{PLATYNUI_ROOT_DESCRIPTOR}}}']  # pyright: ignore[reportOptionalMemberAccess]
            if isinstance(r, UiNodeDescriptor):
                return r(True)

        return None

    @property
    def query_settings(self) -> QuerySettings:
        """The effective query settings: the nearest Robot Framework scope's value, else the
        library-import default.

        `Set Query Settings` stores a full `QuerySettings` instance in ``${PLATYNUI_QUERY_SETTINGS}``
        at the chosen scope, so Robot Framework's own nearest-scope-wins resolution provides the
        precedence (LOCAL over TEST over SUITE) and clears each scope when it ends. This is the base
        the wait loop uses; a descriptor's per-call ``overrides`` are applied on top.
        """
        ctx = EXECUTION_CONTEXTS.current
        if ctx is not None and PLATYNUI_QUERY_SETTINGS in ctx.variables:
            settings = ctx.variables[f'${{{PLATYNUI_QUERY_SETTINGS}}}']
            if isinstance(settings, QuerySettings):
                return settings
        return self._default_query_settings

    @keyword
    def set_root(
        self, descriptor: UiNodeDescriptor | None, scope: Literal['LOCAL', 'TEST', 'SUITE'] = 'LOCAL'
    ) -> UiNodeDescriptor | None:
        """Set the default root that subsequent *relative* selectors resolve against.

        A *relative* selector (``.//``, ``./``, ``..``, ``.``) is resolved against the current root
        and drills *into* it; an *absolute* selector (``//``, ``/``) ignores it and starts at the
        desktop. Roots chain, so you can narrow step by step; ``Set Root    ${None}`` resets to the
        desktop. See `Scoping queries to a container` for the full picture, including scope.

        Args:
            descriptor: The new root — a selector string, a ``UiNode`` or descriptor (e.g. one
                returned by another query), or a root previously returned by this keyword (restored
                unchanged). Pass ``${None}`` to reset to the desktop.
            scope: Lifetime of the root: ``LOCAL`` (default, current test/keyword only), ``TEST``
                (whole test, including called keywords) or ``SUITE`` (every test in the suite).

        Returns:
            UiNodeDescriptor | None: The root set at the same ``scope`` before this call (``None`` if
            none). Pass it back at that scope to restore it.

        Examples:
            | `Set Root`        /app:Application[@Name="Editor"]    scope=SUITE    # whole suite runs in the Editor
            | `Set Root`        .//Dialog[@Name="Save"]    # drill into the Save dialog
            | `Pointer Click`   .//Button[@Name="OK"]    # acts relative to the dialog
            | `Set Root`        ${None}    # reset to the desktop
        """
        variables = EXECUTION_CONTEXTS.current.variables  # pyright: ignore[reportOptionalMemberAccess]
        name = f'${{{PLATYNUI_ROOT_DESCRIPTOR}}}'

        # Set the scope variables directly rather than via BuiltIn(), which would log in the wrong
        # step context and run the selector through Robot's variable-syntax resolution.
        suite_vars = getattr(variables, '_suite', None)  # pyright: ignore[reportUnknownArgumentType]
        test_vars = getattr(variables, '_test', None)  # pyright: ignore[reportUnknownArgumentType]
        if scope == 'LOCAL':
            scope_store = variables.current
        elif scope == 'TEST':
            scope_store = test_vars if test_vars is not None else suite_vars
        else:  # 'SUITE'
            scope_store = suite_vars
        if scope_store is None:  # only if Robot has no suite scope yet (not during keyword execution)
            scope_store = variables.current

        # old_root is read from the requested scope (so a same-scope restore is exact); effective_root
        # is the currently visible root that relative drilling resolves against.
        old_root: UiNodeDescriptor | None = scope_store.get(name)
        effective_root = variables.current.get(name)

        if descriptor is None:
            new_root: UiNodeDescriptor | None = None
        elif descriptor.is_root_binding:
            new_root = descriptor
        else:
            # A context-dependent selector drills into the effective root (captured as parent); an
            # independent one starts fresh.
            query = descriptor.query
            try:
                context_dependent = query is not None and self.runtime.is_context_dependent(query)
            except EvaluationError as e:
                raise InvalidSelectorError(f'Invalid selector for Set Root: {query!r} ({e})') from e
            parent = effective_root if context_dependent and isinstance(effective_root, UiNodeDescriptor) else None
            # A query binding must re-resolve against its captured parent, so never copy the shared
            # descriptor's cached node into it; keep a concrete node only when there is no query.
            node = descriptor.node if descriptor.query is None else None
            new_root = UiNodeDescriptor(node, descriptor.query, self, parent=parent, is_root_binding=True)

        if scope == 'LOCAL':
            variables.set_local(name, new_root)
        elif scope == 'TEST':
            variables.set_test(name, new_root)
        else:  # 'SUITE'
            variables.set_suite(name, new_root)

        return old_root

    @keyword
    def set_query_settings(
        self,
        overrides: QuerySettingsDict | QuerySettings | None = None,
        scope: Literal['LOCAL', 'TEST', 'SUITE'] = 'LOCAL',
    ) -> QuerySettings | None:
        """Set the query wait/retry settings for the given Robot Framework scope.

        These settings govern how the action and read keywords *wait* for their target (see
        `Waiting for elements`): ``timeout`` (how long to keep retrying), ``retry_interval`` (the
        pause between attempts) and ``ignore_exceptions`` (swallow evaluation errors instead of
        failing). Name only the fields you want to change — they are applied over the settings already
        in effect at that scope, so the rest are inherited. The result lives as long as its variable
        ``scope`` and clears itself when that scope ends, exactly like `Set Root` — no teardown to
        remember.

        Like a root, scopes nest: a value set at ``LOCAL`` shadows one at ``TEST`` shadows one at
        ``SUITE`` shadows the library-import defaults. Settings set this way apply to *every* lookup,
        including the re-resolution of a `Set Root` root; a per-keyword ``query_overrides`` only tunes
        that one keyword's own target.

        Args:
            overrides: The fields to change, as a dict (e.g. ``{'timeout': 60}``), or a value returned
                by this keyword to restore it exactly. Pass ``${None}`` to drop this scope's settings
                and fall back to the enclosing scope (the analog of ``Set Root    ${None}``).
            scope: Lifetime of the settings: ``LOCAL`` (default, current test/keyword only), ``TEST``
                (whole test, including called keywords) or ``SUITE`` (every test in the suite).

        Returns:
            QuerySettings | None: The settings set at the same ``scope`` before this call (``None`` if
            none). Pass it back at that scope to restore it.

        Examples:
            | `Set Query Settings`    {'timeout': 60}    scope=SUITE    # whole suite waits up to 60 s
            | ${prev}=    `Set Query Settings`    {'timeout': 5}    # be impatient for a moment
            | `Set Query Settings`    ${prev}    # restore exactly
            | `Set Query Settings`    ${None}    # drop this scope's settings
        """
        variables = EXECUTION_CONTEXTS.current.variables  # pyright: ignore[reportOptionalMemberAccess]
        name = f'${{{PLATYNUI_QUERY_SETTINGS}}}'

        # Read the previous value from the requested scope (so a same-scope restore is exact), mirroring
        # the scope selection in set_root. Setting the scope variables directly avoids BuiltIn() logging
        # in the wrong step context.
        suite_vars = getattr(variables, '_suite', None)  # pyright: ignore[reportUnknownArgumentType]
        test_vars = getattr(variables, '_test', None)  # pyright: ignore[reportUnknownArgumentType]
        if scope == 'LOCAL':
            scope_store = variables.current
        elif scope == 'TEST':
            scope_store = test_vars if test_vars is not None else suite_vars
        else:  # 'SUITE'
            scope_store = suite_vars
        if scope_store is None:  # only if Robot has no suite scope yet (not during keyword execution)
            scope_store = variables.current

        old: QuerySettings | None = scope_store.get(name)
        old = old if isinstance(old, QuerySettings) else None

        new: QuerySettings | None
        if overrides is None:
            new = None  # reset this scope -> fall back to the enclosing scope / library defaults
        elif isinstance(overrides, QuerySettings):
            new = overrides  # restore a value previously returned by this keyword
        else:
            # Merge the partial onto the base visible *from the target scope*, never the globally
            # nearest one: setting a field at a wider scope must not inherit a narrower active scope's
            # siblings. For LOCAL that base is the full nearest-wins chain; for TEST/SUITE it is the
            # value already at that scope (RF copies the suite scope into the test scope, so a TEST
            # base transparently picks up an enclosing SUITE value), else the import default.
            if scope == 'LOCAL':
                base = self.query_settings
            else:
                base = old if old is not None else self._default_query_settings
            new = replace(base, **overrides)  # partial dict -> full instance

        if scope == 'LOCAL':
            variables.set_local(name, new)
        elif scope == 'TEST':
            variables.set_test(name, new)
        else:  # 'SUITE'
            variables.set_suite(name, new)

        return old

    @keyword
    def query(
        self,
        expression: str,
        root: UiNode | None = None,
        only_first: bool = False,
    ) -> Any:
        """Evaluate an XPath 2.0 expression against the live UI tree and return what it selects.

        Returns a list — one entry per match, empty when nothing matches. Pass
        ``only_first=${True}`` for just the first match, which is ``${None}`` when there is none.
        What each entry is — an element, an attribute, or a computed value — is explained in
        `What a query gives you`; how to write the expression is explained in `Finding elements`.

        Unlike the action keywords, `Query` does not wait: it reports the tree as it is at that
        moment and returns immediately, even when nothing matches. Pass ``root`` to scope it to a
        container (see `Scoping queries to a container`).

        | @{buttons}=    `Query`    //Button
        | ${ok}=         `Query`    //Button[@Name="OK"]    only_first=${True}
        | ${count}=      `Query`    count(//Button)    only_first=${True}
        """

        if root is None:
            root = self.root

        self.runtime.clear_cache()
        return self.runtime.evaluate_single(expression, root) if only_first else self.runtime.evaluate(expression, root)

    @keyword
    def wait_until_exists(
        self, descriptor: UiNodeDescriptor, *, query_overrides: QuerySettingsDict | None = None
    ) -> UiNode:
        """Wait until a selector resolves to an element, then return that element.

        This is the explicit form of the wait every action keyword already performs (see
        `Waiting for elements`): it re-evaluates the selector against the live tree every
        ``retry_interval`` and returns the element the moment it appears, or fails once
        ``timeout`` elapses. Reach for it as a synchronization point — wait for a window or
        dialog to open before acting, or capture an element for reuse — where `Query` would
        not wait and an action keyword would also act.

        The keyword is element-only: a selector that resolves to a value or an attribute
        (``count(...)``, ``.../@Name``) fails rather than waiting. A captured element passed
        instead of a selector is simply validated as still present. Per-call waiting is tuned
        with ``query_overrides`` (see `Tuning the wait`).

        Args:
            descriptor: The element to wait for — a selector or an element from `Query`.
            query_overrides: Per-call query settings, e.g. ``{'timeout': 10}``.

        Returns:
            UiNode: The resolved element, once it appears.

        Examples:
            | ${dialog}=    `Wait Until Exists`    Window[@Name="Save As"]
            | `Wait Until Exists`    //control:ProgressBar[@Name="Importing"]    query_overrides={'timeout': 60}
        """
        descriptor.overrides = query_overrides
        settings = replace(self.query_settings, **query_overrides) if query_overrides else self.query_settings
        try:
            return descriptor()
        except ElementNotFoundError:
            raise ElementNotFoundError(
                f'No element matched {descriptor.query!r} within timeout of {settings.timeout} seconds.'
            ) from None

    @keyword
    def wait_until_gone(
        self, descriptor: UiNodeDescriptor, *, query_overrides: QuerySettingsDict | None = None
    ) -> None:
        """Wait until an element is gone — a selector matches nothing, or a captured element is no longer valid.

        The counterpart to `Wait Until Exists`: it polls until the target disappears and then
        returns, or raises ``ElementStillPresentError`` once ``timeout`` elapses with the target
        still there. Pass a *selector* to wait until it matches nothing — the live tree is
        re-evaluated on every attempt — or a *captured element* from `Query` to wait until it
        becomes invalid.

        Whether a captured element ever reports itself gone depends on the accessibility
        provider's liveness check. For a *value* condition — a count dropping to zero, say — use
        `Wait Until Query`; a value-producing selector (``count(...)``) is rejected here rather
        than silently waiting out the timeout. A `Set Root` root that itself vanishes surfaces as
        the root's own lookup error. Per-call waiting is tuned with ``query_overrides``.

        Args:
            descriptor: The target whose disappearance to wait for — a selector or an element
                from `Query`.
            query_overrides: Per-call query settings, e.g. ``{'timeout': 10}``.

        Examples:
            | `Wait Until Gone`    Window[@Name="Please wait"]
            | ${spinner}=    `Query`    //control:ProgressBar[@Name="Loading"]    only_first=${True}
            | `Wait Until Gone`    ${spinner}
            | `Wait Until Gone`    Window[@Name="Save As"]    query_overrides={'timeout': 15}
        """
        settings = replace(self.query_settings, **query_overrides) if query_overrides else self.query_settings
        query = descriptor.query
        start = time.monotonic()
        while True:
            gone = False
            try:
                if query is not None:
                    # Selector: re-evaluate fresh every attempt; never trust descriptor.node
                    # (it may be a node cached on the shared descriptor by a prior keyword).
                    self.runtime.clear_cache()
                    result = self.runtime.evaluate_single(query, self.root)
                    if result is not None and not isinstance(result, UiNode):
                        raise ResultTypeError(
                            f'Wait Until Gone expects an element selector or a captured element; query '
                            f'{query!r} returned a value. Use Wait Until Query for value conditions.'
                        )
                    gone = result is None
                else:
                    # Captured element: poll its liveness directly.
                    node = descriptor.node
                    if node is None:
                        gone = True
                    else:
                        self.runtime.clear_cache()
                        node.invalidate()
                        gone = not node.is_valid()
            except ResultTypeError:
                raise  # usage error — surface immediately, regardless of ignore_exceptions
            except (SystemExit, KeyboardInterrupt):
                raise
            except Exception as e:
                if not settings.ignore_exceptions:
                    raise e
                gone = False  # swallowed error => cannot confirm gone => keep waiting

            if gone:
                return

            if (time.monotonic() - start) > settings.timeout:
                if query is not None:
                    raise ElementStillPresentError(
                        f'Element matching query {query!r} was still present within timeout of '
                        f'{settings.timeout} seconds.'
                    )
                raise ElementStillPresentError(
                    f'Captured element {descriptor.node!r} was still valid within timeout of '
                    f'{settings.timeout} seconds.'
                )

            time.sleep(settings.retry_interval)

    @keyword
    def wait_until_query(
        self,
        expression: str,
        assertion_operator: AssertionOperator | None = None,
        assertion_expected: Any = None,
        assertion_message: str | None = None,
        *,
        root: UiNode | None = None,
        query_overrides: QuerySettingsDict | None = None,
    ) -> Any:
        r"""Wait until an XPath result satisfies an assertion (default: until it is truthy).

        The waiting counterpart to `Query`: instead of a one-shot snapshot, it re-evaluates the
        expression against the live tree every ``retry_interval`` until its result satisfies the
        condition, then returns that result — or fails once ``timeout`` elapses. Use it to wait
        for a computed condition: a row count to settle, a value to reach a target, an attribute
        to flip.

        It takes the same assertion arguments as `Get Attribute` — an operator and an expected
        value (``==``, ``!=``, ``contains``, ``starts``, ``ends``, ``matches``, ``>``, ``<`` …;
        see [https://github.com/MarketSquare/AssertionEngine|AssertionEngine]). With no operator
        it waits until the result is *truthy* — a non-zero number, a non-empty string, a true
        boolean, a present element. Unlike `Get Attribute`, it works on the raw XPath result, so
        ``.../@X`` yields an attribute value, ``count(...)`` a number and ``//X`` an element; a
        missing attribute is an empty result here, not an error.

        The ``then`` operator is not supported (it transforms rather than asserts and cannot
        express a wait) — use ``validate`` for a boolean expression. Per-call waiting is tuned
        with ``query_overrides``.

        Args:
            expression: The XPath 2.0 expression to evaluate.
            assertion_operator: Optional AssertionEngine operator; without one the keyword waits
                for a truthy result.
            assertion_expected: The expected value the operator compares against.
            assertion_message: Optional custom failure message.
            root: Optional context node to evaluate against; defaults to the current root.
            query_overrides: Per-call query settings, e.g. ``{'timeout': 10}``.

        Returns:
            The satisfying result — the raw result without an operator (a value, an attribute or
            an element), or the value returned by the assertion with one.

        Examples:
            | ${count}=    `Wait Until Query`    count(//control:ListItem)    >    0
            | `Wait Until Query`    Window[@Name="Editor"]//Button[@Name="Save"]/@IsEnabled    ==    ${True}
            | `Wait Until Query`    string-join(//control:ListItem/@Name, ", ")    contains    Welcome
            | ${n}=    `Wait Until Query`    count(//control:Window[@Name="Dialog"])    # wait until truthy
        """
        # ``then`` transforms rather than asserts, so it never reports a mismatch and cannot wait.
        if assertion_operator is not None and assertion_operator is AssertionOperator['then']:
            raise ResultTypeError(
                "Wait Until Query cannot wait on the 'then' operator (it transforms rather than asserts). "
                "Use 'validate' for a boolean wait condition, or a comparison operator."
            )

        settings = replace(self.query_settings, **query_overrides) if query_overrides else self.query_settings
        ctx = root if root is not None else self.root
        start = time.monotonic()
        while True:
            satisfied = False
            ret: Any = None
            try:
                self.runtime.clear_cache()
                result = self.runtime.evaluate_single(expression, ctx)
                if assertion_operator is None:
                    ret = result
                    satisfied = bool(result)  # relies on UiNode/EvaluatedAttribute __bool__
                else:
                    ret = verify_assertion(
                        _assertion_value(result), assertion_operator, assertion_expected, assertion_message or ''
                    )
                    satisfied = True
            except (SystemExit, KeyboardInterrupt):
                raise
            except RuntimeError:
                raise  # unknown operator — a programming error, surface immediately
            except (AssertionError, TypeError):
                satisfied = False  # mismatch, or not-yet-comparable early value — keep polling
            except Exception as e:
                if not settings.ignore_exceptions:
                    raise e
                satisfied = False

            if satisfied:
                return ret

            if (time.monotonic() - start) > settings.timeout:
                if assertion_operator is None:
                    raise ResultTypeError(
                        f'Query {expression!r} did not become truthy within timeout of {settings.timeout} seconds.'
                    )
                # Final assertion outside the loop so AssertionEngine's actual-vs-expected message surfaces.
                self.runtime.clear_cache()
                result = self.runtime.evaluate_single(expression, ctx)
                try:
                    verify_assertion(
                        _assertion_value(result), assertion_operator, assertion_expected, assertion_message or ''
                    )
                except AssertionError as ae:
                    raise AssertionError(f'{ae} (within timeout of {settings.timeout} seconds)') from None
                raise ResultTypeError(
                    f'Query {expression!r} did not satisfy the assertion within timeout of {settings.timeout} seconds.'
                )

            time.sleep(settings.retry_interval)

    # Internal helpers
    def _maybe_bring_to_front(
        self,
        descriptor: 'UiNodeDescriptor | None',
        activate: bool | None,
    ) -> None:
        """Bring the target element's window to the foreground if activation is enabled.

        Args:
            descriptor: Optional element descriptor. If None, no action is taken.
            activate: Override for auto_activate. If None, the library-level
                ``auto_activate`` setting is used.
        """
        if descriptor is None:
            return
        should_activate = activate if activate is not None else self.auto_activate
        if should_activate:
            try:
                self.runtime.bring_to_front(descriptor())
            except BareMetalError:
                raise  # Critical error from the library/runtime; propagate it
            except (KeyboardInterrupt, SystemExit):
                raise  # Don't interfere with user-initiated interrupts
            except Exception:
                pass  # Best-effort; don't block the pointer action

    def _resolve_screen_point(
        self,
        descriptor: 'UiNodeDescriptor | None',
        x: float | None,
        y: float | None,
    ) -> Point | None:
        """Resolve absolute screen coordinates from optional descriptor and x/y values.

        Behavior:
        - If only one of x or y is provided, raises ValueError.
        - If a descriptor is provided and x/y are None: uses ActivationPoint when available,
          otherwise the center of Bounds.
        - If a descriptor is provided and x/y are given: treats (x, y) as offsets relative
          to the element's top-left Bounds origin.
        - If no descriptor is provided and x/y are given: treats (x, y) as absolute screen
          coordinates.
        - If neither descriptor nor x/y are provided: returns None. Callers decide whether a
          missing point is an error (pointer_move_to raises) or means "current pointer
          position" (pointer_click/press/release pass None through to the runtime).

        Returns:
        - Point | None: Absolute screen coordinates, or None when no point can be resolved.
        """
        if (x is not None) != (y is not None):
            raise ValueError('Both x and y coordinates must be provided together')

        if descriptor is not None:
            target_node = descriptor()

            # No coordinates provided: auto-resolve from node
            if x is None and y is None:
                try:
                    activation_point = target_node.attribute('ActivationPoint')
                except AttributeNotFoundError:
                    activation_point = None

                if isinstance(activation_point, Point):
                    x = activation_point.x
                    y = activation_point.y
                else:
                    # No ActivationPoint (e.g. containers/aggregates like Desktop):
                    # fall back to the center of the element's bounds.
                    try:
                        bounds = target_node.attribute('Bounds')
                    except AttributeNotFoundError:
                        bounds = None
                    if not isinstance(bounds, Rect):
                        raise ValueError('Node has neither an ActivationPoint nor Bounds to target')

                    center = bounds.center()
                    x = center.x
                    y = center.y

            # Relative coordinates provided: offset from node bounds
            elif x is not None and y is not None:
                try:
                    bounds = target_node.attribute('Bounds')
                except AttributeNotFoundError:
                    bounds = None
                if not isinstance(bounds, Rect):
                    raise ValueError('Node has no bounds to calculate relative coordinates')

                x = bounds.x + x
                y = bounds.y + y

        # At this point, x and y must be resolved
        if x is None or y is None:
            return None

        return Point(x, y)

    @keyword
    def pointer_click(
        self,
        descriptor: UiNodeDescriptor | None = None,
        *,
        button: PointerButtonLike = PointerButton.LEFT,
        x: float | None = None,
        y: float | None = None,
        overrides: PointerOverridesLike | None = None,
        activate: bool | None = None,
        query_overrides: QuerySettingsDict | None = None,
    ) -> None:
        """Click on an element, or at screen coordinates.

        The pointer keywords work out where to act from the element and the optional ``x``/``y``:
        with an element and no ``x``/``y`` the click lands on the element itself (its activation
        point, or the center of its bounds if it has none); with ``x``/``y`` as well, they are an
        offset from the element's top-left corner; with ``x``/``y`` and no element they are absolute
        screen coordinates; with neither, the click happens at the current pointer position. The
        other pointer keywords target the same way.

        Args:
            descriptor: Optional element to click — a selector or an element from `Query`.
            button: Mouse button to use (default LEFT).
            x: X coordinate — absolute without an element, otherwise an offset from it.
            y: Y coordinate — absolute without an element, otherwise an offset from it.
            activate: Bring the element's window to the front first; defaults to the library's
                ``auto_activate``.

        Examples:
            | `Pointer Click`    Window[@Name="Settings"]//Button[@Name="OK"]
            | `Pointer Click`    x=${100}    y=${200}
            | `Pointer Click`    Window[@Name="Settings"]//Button[@Name="OK"]    activate=${False}
        """
        if descriptor is not None:
            descriptor.overrides = query_overrides
        self._maybe_bring_to_front(descriptor, activate)
        point = self._resolve_screen_point(descriptor, x, y)
        self.runtime.pointer_click(point, button, overrides)

    @keyword
    def pointer_multi_click(
        self,
        descriptor: UiNodeDescriptor | None = None,
        *,
        clicks: int = 2,
        button: PointerButtonLike = PointerButton.LEFT,
        x: float | None = None,
        y: float | None = None,
        overrides: PointerOverridesLike | None = None,
        activate: bool | None = None,
        query_overrides: QuerySettingsDict | None = None,
    ) -> None:
        """Click several times in quick succession — a double-click by default.

        Args:
            descriptor: Optional element to target — a selector or an element from `Query`. See
                `Pointer Click` for how a target and ``x``/``y`` are resolved.
            clicks: Number of clicks (default 2, a double-click).
            button: Mouse button to use (default LEFT).
            x: X coordinate — absolute without an element, otherwise an offset from it.
            y: Y coordinate — absolute without an element, otherwise an offset from it.
            activate: Bring the element's window to the front first; defaults to the library's
                ``auto_activate``.

        Examples:
            | `Pointer Multi Click`    Window[@Name="Files"]//item:ListItem[@Name="Open"]
            | `Pointer Multi Click`    x=${100}    y=${200}
            | `Pointer Multi Click`    Window[@Name="Files"]//Text[@Name="File"]    clicks=${3}
        """
        if descriptor is not None:
            descriptor.overrides = query_overrides
        self._maybe_bring_to_front(descriptor, activate)
        point = self._resolve_screen_point(descriptor, x, y)
        self.runtime.pointer_multi_click(point, clicks, button, overrides)

    @keyword
    def pointer_press(
        self,
        descriptor: UiNodeDescriptor | None = None,
        *,
        button: PointerButtonLike = PointerButton.LEFT,
        x: float | None = None,
        y: float | None = None,
        overrides: PointerOverridesLike | None = None,
        activate: bool | None = None,
        query_overrides: QuerySettingsDict | None = None,
    ) -> None:
        """Press a mouse button and hold it down — pair with `Pointer Release` to drag.

        Args:
            descriptor: Optional element to target — a selector or an element from `Query`. See
                `Pointer Click` for how a target and ``x``/``y`` are resolved.
            button: Mouse button to use (default LEFT).
            x: X coordinate — absolute without an element, otherwise an offset from it.
            y: Y coordinate — absolute without an element, otherwise an offset from it.
            activate: Bring the element's window to the front first; defaults to the library's
                ``auto_activate``.

        Examples:
            | `Pointer Press`    Window[@Name="Mixer"]//Slider    x=${10}    y=${5}
        """
        if descriptor is not None:
            descriptor.overrides = query_overrides
        self._maybe_bring_to_front(descriptor, activate)
        point = self._resolve_screen_point(descriptor, x, y)
        self.runtime.pointer_press(point, button, overrides)

    @keyword
    def pointer_release(
        self,
        descriptor: UiNodeDescriptor | None = None,
        *,
        button: PointerButtonLike = PointerButton.LEFT,
        x: float | None = None,
        y: float | None = None,
        overrides: PointerOverridesLike | None = None,
        activate: bool | None = None,
        query_overrides: QuerySettingsDict | None = None,
    ) -> None:
        """Release a mouse button — completes a press, for example to end a drag.

        With a target the pointer is moved there first, then the button is released; without one
        the button is released at the current pointer position.

        Args:
            descriptor: Optional element to target — a selector or an element from `Query`. See
                `Pointer Click` for how a target and ``x``/``y`` are resolved.
            button: Mouse button to release (default LEFT).
            x: X coordinate — absolute without an element, otherwise an offset from it.
            y: Y coordinate — absolute without an element, otherwise an offset from it.
            activate: Bring the element's window to the front first; defaults to the library's
                ``auto_activate``.

        Examples:
            | `Pointer Release`
            | `Pointer Release`    Window[@Name="Editor"]//Canvas    x=${50}    y=${50}
        """
        if descriptor is not None:
            descriptor.overrides = query_overrides
        self._maybe_bring_to_front(descriptor, activate)
        point = self._resolve_screen_point(descriptor, x, y)
        self.runtime.pointer_release(point, button, overrides)

    @keyword
    def pointer_move_to(
        self,
        descriptor: UiNodeDescriptor | None = None,
        *,
        x: float | None = None,
        y: float | None = None,
        overrides: PointerOverridesLike | None = None,
        activate: bool | None = None,
        query_overrides: QuerySettingsDict | None = None,
    ) -> None:
        """Move the pointer onto an element, or to screen coordinates, without clicking.

        Args:
            descriptor: Optional element to target — a selector or an element from `Query`. See
                `Pointer Click` for how a target and ``x``/``y`` are resolved.
            x: X coordinate — absolute without an element, otherwise an offset from it.
            y: Y coordinate — absolute without an element, otherwise an offset from it.
            activate: Bring the element's window to the front first; defaults to the library's
                ``auto_activate``.

        Examples:
            | `Pointer Move To`    x=${400}    y=${300}
            | `Pointer Move To`    Window[@Name="Settings"]//Button[@Name="OK"]
        """
        if descriptor is not None:
            descriptor.overrides = query_overrides
        self._maybe_bring_to_front(descriptor, activate)
        point = self._resolve_screen_point(descriptor, x, y)
        if point is None:
            raise ValueError('Coordinates x and y must be specified either directly or via node')

        self.runtime.pointer_move_to(point, overrides)

    @keyword
    def pointer_scroll(
        self,
        descriptor: UiNodeDescriptor | None = None,
        *,
        direction: ScrollDirection = 'DOWN',
        ticks: int = 1,
        x: float | None = None,
        y: float | None = None,
        overrides: PointerOverridesLike | None = None,
        activate: bool | None = None,
        query_overrides: QuerySettingsDict | None = None,
    ) -> None:
        """Turn the mouse wheel over an element, at coordinates, or where the pointer is.

        Scrolling is expressed as a ``direction`` and a number of wheel ``ticks`` — you never deal
        with signed deltas. With a target the pointer is moved over it first, so the wheel acts on the
        widget under the cursor (the same targeting as `Pointer Click`); pass ``${None}`` with no
        ``x``/``y`` to scroll wherever the pointer currently is.

        One ``tick`` is one mouse-wheel notch (120 units, the cross-platform wheel step). The keyword
        owns the sign and axis, so the requested direction is the one you see: ``DOWN``/``UP`` scroll
        vertically, ``RIGHT``/``LEFT`` horizontally.

        Args:
            descriptor: Optional element to scroll over — a selector or an element from `Query`. See
                `Pointer Click` for how a target and ``x``/``y`` are resolved. Pass ``${None}`` to
                scroll at the current pointer position.
            direction: ``UP``, ``DOWN``, ``LEFT`` or ``RIGHT`` (default ``DOWN``).
            ticks: Number of mouse-wheel notches to turn (default 1).
            x: X coordinate — absolute without an element, otherwise an offset from it.
            y: Y coordinate — absolute without an element, otherwise an offset from it.
            activate: Bring the element's window to the front first; defaults to the library's
                ``auto_activate``.

        Examples:
            | `Pointer Scroll`    //control:List[@Name="Inbox"]    # one notch down over the list
            | `Pointer Scroll`    //control:List[@Name="Inbox"]    direction=DOWN    ticks=${3}
            | `Pointer Scroll`    ${None}    direction=RIGHT    ticks=${2}    # at the current position
            | `Pointer Scroll`    x=${400}    y=${300}    direction=UP
        """
        if descriptor is not None:
            descriptor.overrides = query_overrides
        self._maybe_bring_to_front(descriptor, activate)
        point = self._resolve_screen_point(descriptor, x, y)
        if point is not None:
            self.runtime.pointer_move_to(point, overrides)
        self.runtime.pointer_scroll(_scroll_delta(direction, ticks), overrides)

    @keyword
    @assertable
    def get_pointer_position(self) -> Any:
        """Get the current pointer position on the screen.

        Returns:
            Point: The current screen coordinates of the pointer.

        This keyword is assertable: pass ``assertion_operator`` (and ``assertion_expected``)
        to verify the position.
        """
        return self.runtime.pointer_position()

    @keyword
    def get_element_at_point(self, x: float, y: float) -> Any:
        """Resolve the deepest UI element at the given screen coordinates.

        Uses the platform hit-test: UI Automation ``ElementFromPoint`` on Windows;
        window-manager window selection plus AT-SPI descent on Linux. Returns the
        resolved ``UiNode``, or ``None`` when nothing is at the point. Raises if the
        active platform cannot hit-test (e.g. a generic Wayland session).

        Args:
            x: Screen X coordinate.
            y: Screen Y coordinate.
        """
        return self.runtime.element_at_point(float(x), float(y))

    @keyword
    def focus(
        self,
        descriptor: UiNodeDescriptor,
        *,
        activate: bool | None = None,
        query_overrides: QuerySettingsDict | None = None,
    ) -> None:
        """Set keyboard focus to an element, bringing its window to the front first.

        Waits for its target like the other action keywords. The keyboard keywords focus their own
        target, so you only need this for focus that is not tied to a type action. The element's
        top-level window is raised first so the focus is desktop-wide and not merely within its
        application (see `Bringing windows to the front`); pass ``activate=${False}`` to skip that.

        Args:
            descriptor: The element to focus — a selector or an element from `Query`.
            activate: Bring the element's window to the front first; defaults to the library's
                ``auto_activate``.

        Examples:
            | `Focus`    Window[@Name="Browser"]//Edit[@Name="Search"]
        """
        descriptor.overrides = query_overrides
        self._maybe_bring_to_front(descriptor, activate)
        self.runtime.focus(descriptor())

    @keyword
    def restore_window(self, descriptor: UiNodeDescriptor, *, query_overrides: QuerySettingsDict | None = None) -> None:
        """Restore a window to its previous size and position.

        Returns a minimized or maximized window to the size and position it had before. Waits for
        its target like the other action keywords. The target must be a window that can be
        restored; the keyword fails if it cannot (see `Window control`).

        Args:
            descriptor: The window element to restore.

        Examples:
            | `Restore Window`    Window[@Name="Settings"]
        """
        descriptor.overrides = query_overrides
        node = descriptor()
        node.get_pattern(Restorable).restore()

    @keyword
    def maximize_window(
        self, descriptor: UiNodeDescriptor, *, query_overrides: QuerySettingsDict | None = None
    ) -> None:
        """Maximize a window so it fills the screen.

        Waits for its target like the other action keywords. The target must be a window that can
        be maximized; the keyword fails if it cannot (see `Window control`).

        Args:
            descriptor: The window element to maximize.

        Examples:
            | `Maximize Window`    Window[@Name="Editor"]
        """
        descriptor.overrides = query_overrides
        node = descriptor()
        node.get_pattern(Maximizable).maximize()

    @keyword
    def minimize_window(
        self, descriptor: UiNodeDescriptor, *, query_overrides: QuerySettingsDict | None = None
    ) -> None:
        """Minimize a window to the taskbar.

        Waits for its target like the other action keywords. The target must be a window that can
        be minimized; the keyword fails if it cannot (see `Window control`).

        Args:
            descriptor: The window element to minimize.

        Examples:
            | `Minimize Window`    Window[@Name="Editor"]
        """
        descriptor.overrides = query_overrides
        node = descriptor()
        node.get_pattern(Minimizable).minimize()

    @keyword
    def close_window(self, descriptor: UiNodeDescriptor, *, query_overrides: QuerySettingsDict | None = None) -> None:
        """Close a window, as if the user clicked its close button.

        Waits for its target like the other action keywords. The target must be a window that can
        be closed; the keyword fails if it cannot (see `Window control`).

        Args:
            descriptor: The window element to close.

        Examples:
            | `Close Window`    Window[@Name="Editor"]
        """
        descriptor.overrides = query_overrides
        node = descriptor()
        node.get_pattern(Closeable).close()

    @keyword
    def activate_window(
        self, descriptor: UiNodeDescriptor, *, query_overrides: QuerySettingsDict | None = None
    ) -> None:
        """Bring a window to the front and give it the keyboard focus.

        Waits for its target like the other action keywords. The target must be a window that can
        be activated; the keyword fails if it cannot (see `Window control`).

        Args:
            descriptor: The window element to activate.

        Examples:
            | `Activate Window`    Window[@Name="Editor"]
        """
        descriptor.overrides = query_overrides
        node = descriptor()
        node.get_pattern(Activatable).activate()

    @keyword
    def move_window(
        self, descriptor: UiNodeDescriptor, x: float, y: float, *, query_overrides: QuerySettingsDict | None = None
    ) -> None:
        """Move a window so its top-left corner is at the given screen position.

        Waits for its target like the other action keywords. The target must be a window that can
        be moved; the keyword fails if it cannot (see `Window control`).

        Args:
            descriptor: The window element to move.
            x: The target x coordinate for the window's top-left corner.
            y: The target y coordinate for the window's top-left corner.

        Examples:
            | `Move Window`    Window[@Name="Editor"]    100    200
        """
        descriptor.overrides = query_overrides
        node = descriptor()
        node.get_pattern(Movable).move_to(x, y)

    @keyword
    def resize_window(
        self,
        descriptor: UiNodeDescriptor,
        width: float,
        height: float,
        *,
        query_overrides: QuerySettingsDict | None = None,
    ) -> None:
        """Resize a window to the given width and height.

        Waits for its target like the other action keywords. The target must be a window that can
        be resized; the keyword fails if it cannot (see `Window control`).

        Args:
            descriptor: The window element to resize.
            width: The target width.
            height: The target height.

        Examples:
            | `Resize Window`    Window[@Name="Editor"]    800    600
        """
        descriptor.overrides = query_overrides
        node = descriptor()
        node.get_pattern(Resizable).resize(width, height)

    @keyword
    def move_and_resize_window(
        self,
        descriptor: UiNodeDescriptor,
        x: float,
        y: float,
        width: float,
        height: float,
        *,
        query_overrides: QuerySettingsDict | None = None,
    ) -> None:
        """Move and resize a window in a single step.

        Waits for its target like the other action keywords. The target must be a window that can
        be both moved and resized; the keyword fails if it cannot (see `Window control`).

        Args:
            descriptor: The window element to move and resize.
            x: The target x coordinate for the window's top-left corner.
            y: The target y coordinate for the window's top-left corner.
            width: The target width.
            height: The target height.

        Examples:
            | `Move And Resize Window`    Window[@Name="Editor"]    100    200    800    600
        """
        descriptor.overrides = query_overrides
        node = descriptor()
        node.get_pattern(Movable).move_to(x, y)
        node.get_pattern(Resizable).resize(width, height)

    @keyword
    def bring_to_front(self, descriptor: UiNodeDescriptor, *, query_overrides: QuerySettingsDict | None = None) -> None:
        """Bring an element's window to the front and give it the keyboard focus.

        Pointer actions already do this when ``auto_activate`` is on (the default), so you rarely
        need it directly — reach for it to raise a window deliberately, for example when
        ``auto_activate`` is off. A minimized window is restored first. `Activate Window` does the
        same thing for a window you already have.

        Args:
            descriptor: The element whose window to bring to the front.

        Examples:
            | `Bring To Front`    Window[@Name="Editor"]
        """
        descriptor.overrides = query_overrides
        node = descriptor()
        self.runtime.bring_to_front(node)

    @keyword
    @assertable
    def get_attribute(
        self, descriptor: UiNodeDescriptor, attribute_name: str, *, query_overrides: QuerySettingsDict | None = None
    ) -> Any:
        """Read one attribute of one element, and optionally assert on it in the same call.

        Pass the attribute name bare — ``Name``, ``IsEnabled``, ``Bounds`` — without the leading
        ``@``; a technology-specific value keeps its prefix (``native:...``). The value comes back
        typed: ``@IsEnabled`` as a boolean, ``@Bounds`` as a ``Rect``. Add an assertion operator and
        an expected value to check it, and the keyword fails if the check does not hold. For several
        values at once, or a computed one, use `Query` instead.

        Args:
            descriptor: The element to read from — a selector or an element from `Query`.
            attribute_name: The attribute to read, written bare (or with a ``native:`` prefix).

        Examples:
            | ${enabled}=    `Get Attribute`    Window[@Name="Editor"]//Button[@Name="Save"]    IsEnabled
            | `Get Attribute`    Window[@Name="Editor"]//Button[@Name="Save"]    IsEnabled    ==    ${True}
            | ${bounds}=     `Get Attribute`    Window[@Name="Editor"]//Button[@Name="Save"]    Bounds
        """
        namespace: str | None = None
        if ':' in attribute_name:
            namespace, attribute_name = attribute_name.split(':', 1)
        descriptor.overrides = query_overrides
        node = descriptor()
        return node.attribute(attribute_name, namespace)

    @keyword
    def keyboard_type(
        self,
        descriptor: UiNodeDescriptor | None,
        text: str,
        *,
        overrides: KeyboardOverridesLike | None = None,
        activate: bool | None = None,
        query_overrides: QuerySettingsDict | None = None,
    ) -> None:
        r"""Type a sequence of characters and/or keys.

        If ``descriptor`` is provided, its window is brought to the front and the element focused
        first (see `Bringing windows to the front`).
        Sequences may include plain text and special keys wrapped in angle brackets.
        Use ``+`` to combine modifiers with keys.

        Args:
            descriptor: Optional element to focus before typing. Pass ``${None}`` to type
                into the currently focused element without changing focus.
            text: The character/key sequence to send.
            overrides: Per-call timing overrides, as a dict (see `Input timing and motion`).
            activate: Bring the element's window to the front first; defaults to the library's
                ``auto_activate``.

        Examples:
            | `Keyboard Type`    Window[@Name="Browser"]//Edit[@Name="Search"]    Hello World
            | `Keyboard Type`    Window[@Name="Browser"]//Edit[@Name="Search"]    <Ctrl+A><Delete>
            | `Keyboard Type`    ${None}    Hello\nWorld    # newline supported

        Notes:
            - Special key syntax examples: ``<Ctrl+C>``, ``<Return>``, ``<ESC>``, ``<Shift+Tab>``.
            - For the list of supported key names, see the CLI command ``platynui-cli keyboard list``
              or the Python runtime method ``Runtime.keyboard_known_key_names()``.
            - To omit the descriptor (no focus change), pass ``${None}`` as the first argument in Robot Framework.
        """
        if descriptor is not None:
            descriptor.overrides = query_overrides
            self._maybe_bring_to_front(descriptor, activate)
            target_node = descriptor()
            self.runtime.focus(target_node)
        self.runtime.keyboard_type(text, overrides=overrides)

    @keyword
    def keyboard_press(
        self,
        descriptor: UiNodeDescriptor | None,
        text: str,
        *,
        overrides: KeyboardOverridesLike | None = None,
        activate: bool | None = None,
        query_overrides: QuerySettingsDict | None = None,
    ) -> None:
        """Press (and hold) keys according to a sequence.

        Unlike ``Keyboard Type``, this sends only press events (no release). Use this to
        hold modifiers or keys; pair with ``Keyboard Release`` to complete the action.

        Args:
            descriptor: Optional element to bring to front and focus before pressing.
            text: Sequence of keys, e.g. ``<Ctrl+Alt+T>`` or ``<Shift>``.
            overrides: Per-call timing overrides, as a dict (see `Input timing and motion`).
            activate: Bring the element's window to the front first; defaults to the library's
                ``auto_activate``.

        Examples:
            | `Keyboard Press`     Window[@Name="Terminal"]    <Ctrl+Alt+T>
            | `Keyboard Press`     ${None}    <Ctrl>
            | `Keyboard Release`   ${None}    <Ctrl>
        """
        if descriptor is not None:
            descriptor.overrides = query_overrides
            self._maybe_bring_to_front(descriptor, activate)
            target_node = descriptor()
            self.runtime.focus(target_node)
        self.runtime.keyboard_press(text, overrides=overrides)

    @keyword
    def keyboard_release(
        self,
        descriptor: UiNodeDescriptor | None,
        text: str,
        *,
        overrides: KeyboardOverridesLike | None = None,
        activate: bool | None = None,
        query_overrides: QuerySettingsDict | None = None,
    ) -> None:
        """Release keys according to a sequence.

        Complements ``Keyboard Press`` by releasing keys/modifiers. If you need a full
        press→release cycle for characters or shortcuts, prefer ``Keyboard Type``.

        Args:
            descriptor: Optional element to bring to front and focus before releasing.
            text: Sequence of keys to release, e.g. ``<Ctrl+Alt+T>`` or ``<Ctrl>``.
            overrides: Per-call timing overrides, as a dict (see `Input timing and motion`).
            activate: Bring the element's window to the front first; defaults to the library's
                ``auto_activate``.

        Examples:
            | `Keyboard Press`     Window[@Name="Terminal"]    <Ctrl+Alt>
            | `Keyboard Release`   Window[@Name="Terminal"]    <Ctrl+Alt>
            | `Keyboard Release`   ${None}    <Ctrl+Alt>
        """
        if descriptor is not None:
            descriptor.overrides = query_overrides
            self._maybe_bring_to_front(descriptor, activate)
            target_node = descriptor()
            self.runtime.focus(target_node)
        self.runtime.keyboard_release(text, overrides=overrides)

    @keyword
    def take_screenshot(
        self,
        descriptor: UiNodeDescriptor | None = None,
        filename: Literal['EMBED'] | str = 'platynui-screenshot-{index}.png',
        rect: RectLike | None = None,
        *,
        activate: bool | None = None,
        query_overrides: QuerySettingsDict | None = None,
    ) -> str:
        """Take a screenshot of the entire screen or a specific element.

        Args:
            descriptor: Optional element to capture; its window is brought to the front first so it
                is not covered by another (see `Bringing windows to the front`). If None, captures
                the full screen.
            filename: ``EMBED`` to embed the image directly into the log, or a file name to
                save the PNG under the suite's output directory. A ``{index}`` placeholder in
                the name is replaced with an auto-incrementing counter.
            rect: Optional rectangle area to capture. When a descriptor is given, the rect is
                interpreted relative to the element's bounds.
            activate: Bring the element's window to the front first; defaults to the library's
                ``auto_activate``.

        Returns:
            str: The file name the screenshot was written to, or ``EMBED`` when embedded.

        Examples:
            | `Take Screenshot`    filename=EMBED
            | `Take Screenshot`    filename=full_desktop.png
            | `Take Screenshot`    Window[@Name="Settings"]    filename=settings_window.png
        """
        if descriptor is not None:
            descriptor.overrides = query_overrides
            self._maybe_bring_to_front(descriptor, activate)
            node = descriptor()
            node_rect = cast(Rect, node.attribute('Bounds'))
            if rect is not None:
                rect = Rect.from_like(rect)
                translated_rect = node_rect.translate(rect.x, rect.y)
                rect = Rect(
                    translated_rect.x,
                    translated_rect.y,
                    min(rect.width, node_rect.width - (rect.x)),
                    min(rect.height, node_rect.height - (rect.y)),
                )
            else:
                rect = node_rect

        screenshot = self.runtime.screenshot(rect, 'image/png')

        if filename == 'EMBED':
            logger.info(
                '</td></tr><tr><td colspan="3">'
                '<img alt="screenshot" class="robot-seleniumlibrary-screenshot" '
                f'src="data:image/png;base64,{base64.b64encode(screenshot).decode("utf-8")}" '
                'style="max-width:800px;width:100%"/>',
                html=True,
            )
            return filename
        screenshot_dir = self._screenshot_path
        screenshot_dir.mkdir(parents=True, exist_ok=True)
        if '{index}' in filename:
            filename = filename.replace('{index}', str(self._screenshot_counter))
            self._screenshot_counter += 1
        filepath = screenshot_dir / filename
        with open(filepath, 'wb') as f:
            f.write(screenshot)

        relative_path = filepath.relative_to(screenshot_dir)
        logger.info(
            '</td></tr><tr><td colspan="3">'
            f'<a href="{relative_path}" target="_blank"><img src="{relative_path}" '
            'style="max-width:800px;width:100%"/></a>',
            html=True,
        )

        return filename

    @keyword
    def highlight(
        self,
        descriptor: UiNodeDescriptor | list[UiNodeDescriptor] | None = None,
        *,
        rect: RectLike | list[RectLike] | None = None,
        duration: float = 1.0,
        activate: bool | None = None,
        query_overrides: QuerySettingsDict | None = None,
    ) -> None:
        """Draw a temporary outline around one or more elements — handy for demos and debugging.

        Args:
            descriptor: The element(s) to outline — a selector or an element from `Query`. Takes
                precedence over ``rect`` when both are given.
            rect: Screen rectangle(s) to outline directly; used only when no element is given.
            duration: How long the outline stays on screen, in seconds.
            activate: Bring each element's window to the front first; defaults to the library's
                ``auto_activate``.
        """

        if descriptor is None and rect is None:
            raise ValueError('Either descriptor or rect must be provided for highlighting')

        descriptor_list: list[UiNodeDescriptor] = []
        if isinstance(descriptor, UiNodeDescriptor):
            descriptor_list = [descriptor]
        elif isinstance(descriptor, list):
            descriptor_list = descriptor

        rects: list[Rect] = []
        if descriptor_list:
            for d in descriptor_list:
                d.overrides = query_overrides
                try:
                    self._maybe_bring_to_front(d, activate)
                    r = cast(Rect, d().attribute('Bounds'))
                    rects.append(r)
                except Exception:
                    logger.trace(
                        f'Could not retrieve bounds for descriptor {d.node!r}, skipping highlight for this node'
                    )
                    continue

            self.runtime.highlight(rects, duration * 1000)
            return

        if rect is not None:
            self.runtime.highlight(rect, duration * 1000)  # duration in ms
