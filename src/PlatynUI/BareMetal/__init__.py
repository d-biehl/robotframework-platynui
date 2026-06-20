import base64
import time
from dataclasses import dataclass
from functools import cached_property
from pathlib import Path
from typing import Any, Literal, cast

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

        start_time = time.monotonic()
        result: UiNode | UiValue | EvaluatedAttribute | None = None
        while True:
            try:
                result = self.library.runtime.evaluate_single(self.query, context)
            except (SystemExit, KeyboardInterrupt):
                raise  # Don't interfere with user-initiated interrupts
            except Exception as e:
                if not self.library.query_settings.ignore_exceptions:
                    raise e
            else:
                if result is not None:
                    break

                if (time.monotonic() - start_time) > self.library.query_settings.timeout:
                    raise ElementNotFoundError(
                        f'No UiNode found for UiNodeDescriptor query {self.query!r} within '
                        f'timeout of {self.library.query_settings.timeout} seconds.'
                    )

                time.sleep(self.library.query_settings.retry_interval)
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


@dataclass
class QuerySettings:
    """Settings for query evaluation context."""

    timeout: float
    retry_interval: float
    ignore_exceptions: bool = False


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
    | ``app:`` | a running application (see *Targeting a specific application*) |
    | ``native:`` | raw, technology-specific roles and attributes |

    Attributes have no default namespace (unlike element names), so a standard attribute is written
    bare — ``@Name``, ``@Id``, ``@Bounds`` — and a namespaced one with its prefix (an application's
    process details as ``@app:...``, raw values as ``@native:...``).

    | @{rows}=    `Query`    //List[@Name="Inbox"]/item:ListItem

    = Finding elements =

    `Query` evaluates an XPath 2.0 expression against the live tree. You select an element by its
    role and filter on its attributes in ``[...]`` — exactly, partially, by regular expression, or by
    position:

    | `Pointer Click`    //Button[@Name="OK"]    # exact
    | `Keyboard Type`    //Edit[starts-with(@Name, "Address")]    12 Main St    # partial
    | `Get Attribute`    //CheckBox[matches(@Name, "Option [0-9]+")]    IsEnabled    # regular expression
    | `Pointer Click`    (//Button)[last()]    # by position

    Top-level windows are children of the desktop, so a single leading ``/`` is enough to reach
    them; ``//Window`` would also match windows nested inside others (the *Steps and axes* below
    cover ``/``, ``//`` and the rest):

    | `Pointer Click`    /Window[@Name="Settings"]//Button[@Name="Save"]

    ``=`` is exact and case-sensitive; use ``contains()``, ``starts-with()`` or ``matches()`` for
    partial text. The usual XPath 2.0 string, numeric and boolean functions are available.

    To match on more than one fact, combine conditions with ``and`` / ``or`` in one predicate, or
    chain predicates. A boolean attribute such as ``@IsVisible`` is compared with ``true()`` or
    ``false()`` — not the strings ``"true"``/``"false"``:

    | `Pointer Click`    //Button[@Name="Save" and @IsVisible=true()]    # named *and* visible
    | `Pointer Click`    //Button[@Name="OK" or @Name="Yes"]    # either label

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

    A leading ``/`` or ``//`` starts from the desktop, so the path is *absolute*; a leading ``.`` or
    ``..`` starts from the current root, so it is *relative* — this is the distinction
    *Scoping queries to a container* builds on.

    When you need a direction the shorthand has no symbol for — an ancestor, a sibling — name the
    axis in full as ``axis::Role``:

    | ${dialog}=    `Query`    //Button[@Name="OK"]/ancestor::Dialog    only_first=${True}
    | @{later}=     `Query`    //item:ListItem[@Name="Today"]/following-sibling::item:ListItem

    The named axes are ``child::``, ``descendant::``, ``parent::``, ``ancestor::``,
    ``ancestor-or-self::``, ``following-sibling::``, ``preceding-sibling::`` and ``following::``.

    = Acting on elements =

    The action keywords — pointer (click, press, move), keyboard, focus, window control (move,
    resize, minimize, maximize, activate, close), screenshots and highlight — take either a selector
    or an element you captured with `Query`. Each waits for its target (see *Waiting for elements*):

    | `Pointer Click`     //Button[@Name="New"]
    | `Keyboard Type`     //Edit[@Name="Title"]    Q3 Report
    | `Maximize Window`   //Window[@Name="Editor"]

    Or capture an element once with `Query` and reuse it — every keyword takes it the same way:

    | ${save}=    `Query`    //Button[@Name="Save"]    only_first=${True}
    | `Pointer Click`    ${save}
    | `Highlight`        ${save}

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

    | ${enabled}=    `Get Attribute`    //Button[@Name="Save"]    IsEnabled
    | `Get Attribute`    //Button[@Name="Save"]         IsEnabled    ==    ${True}
    | `Get Attribute`    //CheckBox[@Name="Dark mode"]  IsEnabled    ==    ${False}

    Operators include ``==``, ``!=``, ``contains``, ``starts``, ``ends`` and ``matches`` (see
    [https://github.com/MarketSquare/AssertionEngine|AssertionEngine]). For several values at once,
    or a computed one, read them through `Query` instead.

    = Scoping queries to a container =

    Every query runs against a *context node*, the desktop by default. Scope it to a container you
    found and relative selectors (``.//``, ``./``, axes like ``child::``) search inside it; an
    absolute ``//`` ignores the context and starts again at the desktop.

    For one query, pass ``root``:

    | ${inbox}=    `Query`    //List[@Name="Inbox"]    only_first=${True}
    | @{items}=    `Query`    .//item:ListItem    root=${inbox}

    `Set Root` makes a root the default for everything that follows, so you stop repeating a long
    prefix. A new `Set Root` is itself resolved against the current one: a relative root narrows it
    step by step (``..`` widens again), an absolute root switches away. It returns the previous root,
    and ``Set Root    ${None}`` clears it back to the desktop:

    | `Set Root`        //app:Application[@Name="Editor"]    # work inside the Editor from here on
    | `Set Root`        .//Dialog[@Name="Save"]              # narrow to its Save dialog
    | `Pointer Click`   .//Button[@Name="Save"]

    == Scope ==

    A root lives as long as the Robot Framework variable it is kept in; ``scope`` chooses which, and
    each scope clears itself when it ends — no teardown to remember:

    | = scope = | = The root applies to = |
    | ``LOCAL`` (default) | the current test or keyword, not the keywords it calls |
    | ``TEST`` | the whole test, including called keywords |
    | ``SUITE`` | every test in the suite |

    | `Set Root`    //app:Application[@Name="Editor"]    scope=SUITE

    The root stores the *query*, not a fixed node, so it re-resolves against the live tree and keeps
    working even after its window closes and reopens. (It lives in ``${PLATYNUI_ROOT_DESCRIPTOR}``,
    but set it through `Set Root` — the raw variable holds an internal descriptor.)

    = Targeting a specific application =

    When several programs are open, anchor your selectors to the program itself so they cannot drift
    into the wrong one. Each application is an *application element* in the ``app:`` namespace, with
    the windows and elements it owns beneath it, so matching its name scopes everything below it to
    that program. Either prefix a single query with it:

    | `Pointer Click`    //app:Application[@Name="Editor"]//Button[@Name="Save"]

    or make it the root, so every relative selector that follows stays inside that program:

    | `Set Root`        //app:Application[@Name="Editor"]
    | `Pointer Click`   .//Button[@Name="Save"]

    The node also carries the process. If you launched the program and know its process id, matching
    on it pins the query to that one instance even when several copies run (``ProcessId`` is a number
    — no prefix, no quotes; ``Start Process`` and ``Get Process Id`` are Robot Framework's built-in
    Process keywords):

    | ${proc}=    Start Process     editor
    | ${pid}=     Get Process Id    ${proc}
    | `Set Root`    //app:Application[@ProcessId=${pid}]

    = Waiting for elements =

    Action and read keywords wait for their target: while it is missing the lookup retries for up to
    30 seconds before the keyword fails, so you rarely need explicit sleeps. `Query` is the exception
    — it reports the tree as it is at that moment and returns immediately, even when nothing matches.

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
    | `Keyboard Type`    //Edit[@Name="PIN"]    1234    overrides={'between_keys_delay_ms': 200}

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
    | `Pointer Click`    //Button[@Name="Save"]    overrides={'speed_factor': 0.3}

    = A short example =

    | `Pointer Click`     //Button[@Name="New"]                      # start a new document
    | `Keyboard Type`     ${None}    Q3 Report                       # type into the focused field
    | `Get Attribute`     //Button[@Name="Save"]    IsEnabled    ==    ${True}    # Save is enabled now
    | `Take Screenshot`
    """

    def __init__(
        self,
        *,
        keyboard_profile: KeyboardProfileLike | None = None,
        pointer_settings: PointerSettingsLike | None = None,
        pointer_profile: PointerProfileLike | None = None,
        auto_activate: bool = True,
        use_mock: bool = False,
    ) -> None:
        """Import the library, optionally tuning window activation and input behaviour.

        | Library    PlatynUI.BareMetal

        All arguments are optional. ``auto_activate`` governs window raising; the three *profile*
        arguments set the default timing and motion of generated input, each given as a dict of the
        fields you want to change (see *Input timing and motion*):

        | = Argument = | = What it does = |
        | ``auto_activate`` | bring an element's window to the front before a pointer action — default ``True`` |
        | ``keyboard_profile`` | keyboard *timing*: delays around key presses, chords and whole sequences |
        | ``pointer_settings`` | pointer *behaviour*: double-click interval and size, and the default button |
        | ``pointer_profile`` | pointer *motion*: speed, acceleration, curve, overshoot and jitter |

        | Library    PlatynUI.BareMetal    auto_activate=${False}    pointer_profile={'speed_factor': 0.5}

        ``use_mock`` exists only for PlatynUI's own development and test suites — it drives a built-in
        stand-in tree instead of the real desktop. Leave it at its default; it is not meant for
        automating applications.
        """
        super().__init__([])
        self._screenshot_counter = 1
        self.use_mock = use_mock
        self.auto_activate = auto_activate
        self.query_settings = QuerySettings(30, 0.1, False)
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
        if self.use_mock:
            return Runtime.new_with_mock()

        return Runtime()

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

    @keyword
    def set_root(
        self, descriptor: UiNodeDescriptor | None, scope: Literal['LOCAL', 'TEST', 'SUITE'] = 'LOCAL'
    ) -> UiNodeDescriptor | None:
        """Set the default root that subsequent *relative* selectors resolve against.

        A *relative* selector (``.//``, ``./``, ``..``, ``.``) is resolved against the current root
        and drills *into* it; an *absolute* selector (``//``, ``/``) ignores it and starts at the
        desktop. Roots chain, so you can narrow step by step; ``Set Root    ${None}`` resets to the
        desktop. See *Scoping queries to a container* for the full picture, including scope.

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
            | `Set Root`        //app:Application[@Name="Editor"]    scope=SUITE    # whole suite runs in the Editor
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
        *What a query gives you*; how to write the expression is explained in *Finding elements*.

        Unlike the action keywords, `Query` does not wait: it reports the tree as it is at that
        moment and returns immediately, even when nothing matches. Pass ``root`` to scope it to a
        container (see *Scoping queries to a container*).

        | @{buttons}=    `Query`    //Button
        | ${ok}=         `Query`    //Button[@Name="OK"]    only_first=${True}
        | ${count}=      `Query`    count(//Button)    only_first=${True}
        """

        if root is None:
            root = self.root

        self.runtime.clear_cache()
        return self.runtime.evaluate_single(expression, root) if only_first else self.runtime.evaluate(expression, root)

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
            | `Pointer Click`    //Button[@Name="OK"]
            | `Pointer Click`    x=${100}    y=${200}
            | `Pointer Click`    //Button[@Name="OK"]    activate=${False}
        """
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
            | `Pointer Multi Click`    //item:ListItem[@Name="Open"]
            | `Pointer Multi Click`    x=${100}    y=${200}
            | `Pointer Multi Click`    //Text[@Name="File"]    clicks=${3}
        """
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
            | `Pointer Press`    //Slider    x=${10}    y=${5}
        """
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
            | `Pointer Release`    //Canvas    x=${50}    y=${50}
        """
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
            | `Pointer Move To`    //Button[@Name="OK"]
        """
        self._maybe_bring_to_front(descriptor, activate)
        point = self._resolve_screen_point(descriptor, x, y)
        if point is None:
            raise ValueError('Coordinates x and y must be specified either directly or via node')

        self.runtime.pointer_move_to(point, overrides)

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
    def focus(self, descriptor: UiNodeDescriptor) -> None:
        """Set keyboard focus to an element, bringing its window to the front first.

        Waits for its target like the other action keywords. The keyboard keywords focus their own
        target, so you only need this for focus that is not tied to a type action.

        Args:
            descriptor: The element to focus — a selector or an element from `Query`.

        Examples:
            | `Focus`    //Edit[@Name="Search"]
        """
        self.runtime.focus(descriptor())

    @keyword
    def restore_window(self, descriptor: UiNodeDescriptor) -> None:
        """Restore a window to its previous size and position.

        Returns a minimized or maximized window to the size and position it had before. Waits for
        its target like the other action keywords. The target must be a window that can be
        restored; the keyword fails if it cannot.

        Args:
            descriptor: The window element to restore.

        Examples:
            | `Restore Window`    //Window[@Name="Settings"]
        """
        node = descriptor()
        node.get_pattern(Restorable).restore()

    @keyword
    def maximize_window(self, descriptor: UiNodeDescriptor) -> None:
        """Maximize a window so it fills the screen.

        Waits for its target like the other action keywords. The target must be a window that can
        be maximized; the keyword fails if it cannot.

        Args:
            descriptor: The window element to maximize.

        Examples:
            | `Maximize Window`    //Window[@Name="Editor"]
        """
        node = descriptor()
        node.get_pattern(Maximizable).maximize()

    @keyword
    def minimize_window(self, descriptor: UiNodeDescriptor) -> None:
        """Minimize a window to the taskbar.

        Waits for its target like the other action keywords. The target must be a window that can
        be minimized; the keyword fails if it cannot.

        Args:
            descriptor: The window element to minimize.

        Examples:
            | `Minimize Window`    //Window[@Name="Editor"]
        """
        node = descriptor()
        node.get_pattern(Minimizable).minimize()

    @keyword
    def close_window(self, descriptor: UiNodeDescriptor) -> None:
        """Close a window, as if the user clicked its close button.

        Waits for its target like the other action keywords. The target must be a window that can
        be closed; the keyword fails if it cannot.

        Args:
            descriptor: The window element to close.

        Examples:
            | `Close Window`    //Window[@Name="Editor"]
        """
        node = descriptor()
        node.get_pattern(Closeable).close()

    @keyword
    def activate_window(self, descriptor: UiNodeDescriptor) -> None:
        """Bring a window to the front and give it the keyboard focus.

        Waits for its target like the other action keywords. The target must be a window that can
        be activated; the keyword fails if it cannot.

        Args:
            descriptor: The window element to activate.

        Examples:
            | `Activate Window`    //Window[@Name="Editor"]
        """
        node = descriptor()
        node.get_pattern(Activatable).activate()

    @keyword
    def move_window(self, descriptor: UiNodeDescriptor, x: float, y: float) -> None:
        """Move a window so its top-left corner is at the given screen position.

        Waits for its target like the other action keywords. The target must be a window that can
        be moved; the keyword fails if it cannot.

        Args:
            descriptor: The window element to move.
            x: The target x coordinate for the window's top-left corner.
            y: The target y coordinate for the window's top-left corner.

        Examples:
            | `Move Window`    //Window[@Name="Editor"]    100    200
        """
        node = descriptor()
        node.get_pattern(Movable).move_to(x, y)

    @keyword
    def resize_window(self, descriptor: UiNodeDescriptor, width: float, height: float) -> None:
        """Resize a window to the given width and height.

        Waits for its target like the other action keywords. The target must be a window that can
        be resized; the keyword fails if it cannot.

        Args:
            descriptor: The window element to resize.
            width: The target width.
            height: The target height.

        Examples:
            | `Resize Window`    //Window[@Name="Editor"]    800    600
        """
        node = descriptor()
        node.get_pattern(Resizable).resize(width, height)

    @keyword
    def move_and_resize_window(
        self, descriptor: UiNodeDescriptor, x: float, y: float, width: float, height: float
    ) -> None:
        """Move and resize a window in a single step.

        Waits for its target like the other action keywords. The target must be a window that can
        be both moved and resized; the keyword fails if it cannot.

        Args:
            descriptor: The window element to move and resize.
            x: The target x coordinate for the window's top-left corner.
            y: The target y coordinate for the window's top-left corner.
            width: The target width.
            height: The target height.

        Examples:
            | `Move And Resize Window`    //Window[@Name="Editor"]    100    200    800    600
        """
        node = descriptor()
        node.get_pattern(Movable).move_to(x, y)
        node.get_pattern(Resizable).resize(width, height)

    @keyword
    def bring_to_front(self, descriptor: UiNodeDescriptor) -> None:
        """Bring an element's window to the front and give it the keyboard focus.

        Pointer actions already do this when ``auto_activate`` is on (the default), so you rarely
        need it directly — reach for it to raise a window deliberately, for example when
        ``auto_activate`` is off. A minimized window is restored first. `Activate Window` does the
        same thing for a window you already have.

        Args:
            descriptor: The element whose window to bring to the front.

        Examples:
            | `Bring To Front`    //Window[@Name="Editor"]
        """
        node = descriptor()
        self.runtime.bring_to_front(node)

    @keyword
    @assertable
    def get_attribute(self, descriptor: UiNodeDescriptor, attribute_name: str) -> Any:
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
            | ${enabled}=    `Get Attribute`    //Button[@Name="Save"]    IsEnabled
            | `Get Attribute`    //Button[@Name="Save"]    IsEnabled    ==    ${True}
            | ${bounds}=     `Get Attribute`    //Button[@Name="Save"]    Bounds
        """
        namespace: str | None = None
        if ':' in attribute_name:
            namespace, attribute_name = attribute_name.split(':', 1)
        node = descriptor()
        return node.attribute(attribute_name, namespace)

    @keyword
    def keyboard_type(
        self, descriptor: UiNodeDescriptor | None, text: str, *, overrides: KeyboardOverridesLike | None = None
    ) -> None:
        r"""Type a sequence of characters and/or keys.

        If ``descriptor`` is provided, the element is brought to front and focused first.
        Sequences may include plain text and special keys wrapped in angle brackets.
        Use ``+`` to combine modifiers with keys.

        Args:
            descriptor: Optional element to focus before typing. Pass ``${None}`` to type
                into the currently focused element without changing focus.
            text: The character/key sequence to send.
            overrides: Per-call timing overrides, as a dict (see *Input timing and motion*).

        Examples:
            | `Keyboard Type`    //Edit[@Name="Search"]    Hello World
            | `Keyboard Type`    //Edit[@Name="Search"]    <Ctrl+A><Delete>
            | `Keyboard Type`    ${None}    Hello\nWorld    # newline supported

        Notes:
            - Special key syntax examples: ``<Ctrl+C>``, ``<Return>``, ``<ESC>``, ``<Shift+Tab>``.
            - For the list of supported key names, see the CLI command ``platynui-cli keyboard list``
              or the Python runtime method ``Runtime.keyboard_known_key_names()``.
            - To omit the descriptor (no focus change), pass ``${None}`` as the first argument in Robot Framework.
        """
        if descriptor is not None:
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
    ) -> None:
        """Press (and hold) keys according to a sequence.

        Unlike ``Keyboard Type``, this sends only press events (no release). Use this to
        hold modifiers or keys; pair with ``Keyboard Release`` to complete the action.

        Args:
            descriptor: Optional element to focus before pressing.
            text: Sequence of keys, e.g. ``<Ctrl+Alt+T>`` or ``<Shift>``.
            overrides: Per-call timing overrides, as a dict (see *Input timing and motion*).

        Examples:
            | `Keyboard Press`     //Window[@Name="Terminal"]    <Ctrl+Alt+T>
            | `Keyboard Press`     ${None}    <Ctrl>
            | `Keyboard Release`   ${None}    <Ctrl>
        """
        if descriptor is not None:
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
    ) -> None:
        """Release keys according to a sequence.

        Complements ``Keyboard Press`` by releasing keys/modifiers. If you need a full
        press→release cycle for characters or shortcuts, prefer ``Keyboard Type``.

        Args:
            descriptor: Optional element to focus before releasing.
            text: Sequence of keys to release, e.g. ``<Ctrl+Alt+T>`` or ``<Ctrl>``.
            overrides: Per-call timing overrides, as a dict (see *Input timing and motion*).

        Examples:
            | `Keyboard Press`     //Window[@Name="Terminal"]    <Ctrl+Alt>
            | `Keyboard Release`   //Window[@Name="Terminal"]    <Ctrl+Alt>
            | `Keyboard Release`   ${None}    <Ctrl+Alt>
        """
        if descriptor is not None:
            target_node = descriptor()
            self.runtime.focus(target_node)
        self.runtime.keyboard_release(text, overrides=overrides)

    @keyword
    def take_screenshot(
        self,
        descriptor: UiNodeDescriptor | None = None,
        filename: Literal['EMBED'] | str = 'platynui-screenshot-{index}.png',
        rect: RectLike | None = None,
    ) -> str:
        """Take a screenshot of the entire screen or a specific element.

        Args:
            descriptor: Optional element to capture. If None, captures the full screen.
            filename: ``EMBED`` to embed the image directly into the log, or a file name to
                save the PNG under the suite's output directory. A ``{index}`` placeholder in
                the name is replaced with an auto-incrementing counter.
            rect: Optional rectangle area to capture. When a descriptor is given, the rect is
                interpreted relative to the element's bounds.

        Returns:
            str: The file name the screenshot was written to, or ``EMBED`` when embedded.

        Examples:
            | `Take Screenshot`    filename=EMBED
            | `Take Screenshot`    filename=full_desktop.png
            | `Take Screenshot`    //Window[@Name="Settings"]    filename=settings_window.png
        """
        if descriptor is not None:
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
    ) -> None:
        """Draw a temporary outline around one or more elements — handy for demos and debugging.

        Args:
            descriptor: The element(s) to outline — a selector or an element from `Query`. Takes
                precedence over ``rect`` when both are given.
            rect: Screen rectangle(s) to outline directly; used only when no element is given.
            duration: How long the outline stays on screen, in seconds.
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
                try:
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
