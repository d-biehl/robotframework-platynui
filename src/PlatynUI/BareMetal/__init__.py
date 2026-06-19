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


class UiNodeDescriptor:
    """Descriptor wrapper allowing lazy resolution of a UiNode from a query string.

    The descriptor either holds a concrete UiNode or an expression string that will be
    evaluated using the associated BareMetal library instance when called.
    """

    def __init__(self, node: UiNode | None, query: str | None, library: 'BareMetal') -> None:
        self.node = node
        self.query = query
        self.library = library

    def __call__(self, no_root: bool = False) -> UiNode:
        if isinstance(self.node, UiNode):
            if self.node.is_valid():
                return self.node

        if self.query is None:
            raise NoQueryError('UiNodeDescriptor has no query to resolve the node')

        start_time = time.monotonic()
        result: UiNode | UiValue | EvaluatedAttribute | None = None
        while True:
            try:
                result = self.library.runtime.evaluate_single(self.query, self.library.root if not no_root else None)
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

    BareMetal is PlatynUI's low-level keyword surface. It drives real applications through the
    operating system's accessibility services — UI Automation on Windows and AT-SPI2 on Linux — so
    your tests work with genuine UI controls rather than screen pixels or image matching.

    The whole desktop is presented as a single tree of elements that you query with XPath,
    regardless of the underlying platform. From a query you locate the elements you want — and then
    act on them, or read and evaluate their data.

    Because elements are addressed by their role and attributes — not only their name or a stable
    id, but also their state, geometry, and even raw technology-specific (native) values — rather
    than by fixed coordinates, selectors keep matching even when a window moves or is resized.

    = Querying the UI tree =

    `Query` is the heart of the library: you give it an XPath 2.0 expression and it evaluates it
    against the *live* UI tree, so it does far more than find an element. An element's name in the
    path is its *role* (``Button``, ``Window``, ``CheckBox`` ...) and its data is exposed as
    *attributes* such as ``@Name`` and ``@Id`` (see *Common attributes* below). From a single
    expression you can locate one element or many, count them, read an attribute's value across many
    elements at once, or compute a value or condition:

    | ${ok}=      | Query | //Button[@Name="OK"] | only_first=${True} |
    | @{buttons}= | Query | //Button |
    | ${n}=       | Query | count(//Button) | only_first=${True} |
    | @{names}=   | Query | //Button/@Name |
    | ${joined}=  | Query | string-join(//Button/@Name, ", ") | only_first=${True} |

    == What a query returns ==

    An XPath query does not return a single element — it produces a *sequence* of results, the
    XPath 2.0 equivalent of a result set. The engine builds that sequence lazily: it walks the tree
    and, each time the expression matches, yields that result and carries on searching. Robot
    Framework has no lazy or streaming value, so `Query` collects the whole sequence and hands it
    back as an ordinary **list** — empty when nothing matched, one entry per match otherwise. You
    can therefore loop over it like any other Robot Framework list:

    | @{buttons}= | Query | //Button |
    | FOR | ${button} | IN | @{buttons} |
    |     | Pointer Click | ${button} |
    | END |

    Most of the time you want exactly one element, so pass ``only_first=${True}``. `Query` then
    returns just the first result — stopping the search as soon as it is found — or ``${None}`` if
    the sequence is empty, which is also how you check that something is absent. `Query` never
    waits: it always reports the *current* state of the tree and returns immediately, even when
    nothing matches yet.

    A result is one of three kinds, depending on what the expression selects.

    *An element* (a node), when the last step selects elements — ``//Button`` or
    ``.../parent::*``. This is a live handle to the UI element: pass it to any keyword that expects
    an element, and read its basics directly with Robot Framework's variable syntax — ``${el.role}``,
    ``${el.name}``, ``${el.id}`` (the stable id, empty if the application sets none) and
    ``${el.runtime_id}``.

    | ${win}=       | Query | /Window[@Name="Settings"] | only_first=${True} |
    | Log           | ${win.role} at ${win.runtime_id} |
    | Pointer Click | ${win} |

    *An attribute*, when the expression ends in an attribute step (``.../@Name``). This is not a
    bare string but an object describing one attribute of one element: ``${attr.value}`` is its
    value, ``${attr.name}`` and ``${attr.namespace}`` tell you which attribute it is, and
    ``${attr.owner()}`` is the element it was read from. The owner matters when you read an
    attribute across many elements at once — each result still knows where it came from:

    | @{names}= | Query | //item:ListItem/@item:Name |
    | FOR | ${a} | IN | @{names} |
    |     | Log | ${a.value} belongs to ${a.owner().name} |
    | END |

    The value is typed, not always a string: ``@Name`` gives a string, ``@IsEnabled`` a boolean,
    and ``@Bounds`` a ``Rect`` (with ``x``, ``y``, ``width`` and ``height``) — so ``${attr.value}``
    is ready to use as the right type, without parsing.

    *A value*, when the expression computes something rather than selecting nodes —
    ``count(//Button)``, ``string-join(//Button/@Name, ", ")``, or a comparison such as
    ``count(//Window[@Name="Error"]) = 0``. You get the plain number, string or boolean back, ready
    to use or to assert on.

    == Searching the tree ==

    A path is a series of steps separated by ``/``. ``//`` matches at any depth below the context,
    a single ``/`` matches a direct child, ``.`` is the current element, ``..`` its parent, and
    ``*`` matches any role. Top-level windows are direct children of the desktop, so address them
    with a single leading ``/`` — ``//`` would scan the whole tree and can also match windows
    nested deeper:

    | Pointer Click | /Window[@Name="Settings"]//Button[@Name="Save"] |
    | ${first}=     | Query | /Window[@Name="Settings"]/*[1] | only_first=${True} |
    | @{rows}=      | Query | //control:List[@Name="Inbox"]/item:ListItem |

    == Scoping a query to a parent ==

    Every query is evaluated against a *context node* — by default the desktop. Pass an element you
    captured earlier as the ``root`` argument and the query runs against that element instead, which
    is the natural way to find a container once and then drill into it:

    | ${list}=  | Query | //control:List[@Name="Inbox"] | only_first=${True} |
    | @{items}= | Query | .//item:ListItem | root=${list} |
    | ${first}= | Query | .//item:ListItem | root=${list} | only_first=${True} |

    Only *relative* expressions are scoped to the ``root``: ``.//`` and ``./`` (and named axes such
    as ``child::``) search inside it. An *absolute* ``//`` still starts at the desktop and ignores
    the ``root`` — ``//item:ListItem`` returns every list item on the desktop, not just the ones
    under ``${list}``. `Set Root` applies the same idea as a persistent default: set once, it scopes
    every following query (and the elements the action keywords resolve) until you change or clear
    it.

    == Matching with predicates ==

    A predicate in ``[...]`` filters a step. Match exactly, partially, by regular expression, by
    several conditions, or by position:

    | Pointer Click | //Button[@Name="OK"] |
    | Pointer Click | //Button[starts-with(@Name, "Save")] |
    | Pointer Click | //Button[contains(@Name, "Export")] |
    | Pointer Click | //CheckBox[matches(@Name, "Option [0-9]+")] |
    | Pointer Click | //Button[@Name="OK" or @Name="Ok"] |
    | Pointer Click | (//Button)[1] |
    | Pointer Click | (//Button)[last()] |

    Besides ``count()``, ``contains()``, ``starts-with()`` and ``matches()`` (regular expressions),
    the usual XPath 2.0 string, numeric and boolean functions are available.

    A plain ``=`` compares the whole value exactly and is case-sensitive, so a different casing or a
    stray space will not match — use ``contains()`` or ``starts-with()`` for partial text.

    == Namespaces ==

    Element and attribute names live in one of four namespaces, written as a prefix:

    | = Prefix = | = Selects = |
    | ``control:`` | controls; the *default*, so ``//Button`` equals ``//control:Button`` |
    | ``item:`` | items inside containers: ``ListItem``, ``MenuItem``, ``TabItem``, ``TableCell`` ... |
    | ``app:`` | a running application/process; groups its windows and controls |
    | ``native:`` | raw, technology-specific roles and attributes |

    An attribute usually lives in the same namespace as its element. Control attributes are written
    without a prefix (``@Name``, ``@Id``); an item's, application's or native attribute carries its
    prefix — ``@item:Name``, ``@app:Name``:

    | Pointer Click | //item:ListItem[@item:Name="Run Tests"] |

    == Targeting a specific application ==

    The tree contains more than windows and controls: every running application is itself a node, in
    the ``app:`` namespace. The windows and controls a program owns appear beneath its
    ``app:Application`` node — so the same window is reachable two ways, directly under the desktop
    and under the application that owns it.

    That second path is what the ``app:`` namespace is for. An ``app:Application`` node identifies
    the *program* — by its name or process — independently of how many windows it has or what they
    are titled. So when several programs are open, or one program owns several windows, anchoring to
    its application node is the reliable way to keep a query, or the root (`Set Root`), inside that
    one program:

    | Pointer Click | //app:Application[@app:Name="Notepad"]//Button[@Name="Save"] |
    | Set Root      | //app:Application[@app:Name="Notepad"] |

    Because the node represents the process, it also exposes process details such as its name,
    process id and executable path. When you started the program yourself and know its process id,
    matching on that id pins every query to exactly that instance — even when several copies of the
    same program are running:

    | ${handle}= | Start Process | myapp.exe |
    | ${pid}=    | Get Process Id | ${handle} |
    | Set Root   | //app:Application[@ProcessId=${pid}] |

    (``ProcessId`` has no namespace prefix and is a number — write ``@ProcessId=${pid}``, without a
    prefix or quotes.)

    == Navigating with axes ==

    Steps can walk in any direction, which lets you start from an element you can name and reach a
    related one — for instance the window that contains a button, an element's parent, or its
    siblings:

    | ${win}=  | Query | //Button[@Name="OK"]/ancestor::control:Window | only_first=${True} |
    | ${list}= | Query | //item:ListItem[@item:Name="Run Tests"]/parent::* | only_first=${True} |
    | @{rest}= | Query | //item:ListItem[@item:Name="Inbox"]/following-sibling::item:ListItem |

    Other axes include ``child::``, ``descendant::``, ``ancestor-or-self::``, ``preceding-sibling::``
    and ``following::``.

    == Reading values ==

    To read one attribute of a single element, `Get Attribute` is usually simplest — and it can
    assert on the value (see *Assertions*):

    | ${name}= | Get Attribute | /Window[@Name="Settings"] | Name |

    Inside a query you can select an attribute and read it with ``${attr.value}``, or compute a
    value with ``count(...)``, ``string-join(...)`` and the other XPath functions (see *What a query
    returns*):

    | ${attr}= | Query | /Window[@Name="Settings"]/@Id | only_first=${True} |
    | Log      | ${attr.value} |

    == Common attributes ==

    The attributes you will reach for most in predicates and with `Get Attribute`:

    | = Attribute = | = Meaning = |
    | ``@Name`` | the visible label or caption |
    | ``@Id`` | a stable, language-independent identifier — prefer it when it is set |
    | ``@Bounds`` | the element's screen rectangle |
    | ``@IsVisible``, ``@IsEnabled`` | whether the element is shown / can be interacted with |

    Which attributes an element has depends on the kind of element, so not every attribute is
    available everywhere. Role and attribute names also depend on the application and platform; use
    the PlatynUI Inspector to discover the ones your target exposes.

    = Acting on elements =

    Once a query has located an element, the other keywords act on it: move the pointer and click,
    type on the keyboard, set focus, control windows (move, resize, minimize, maximize, activate,
    close), capture screenshots, and highlight it. Wherever a keyword expects an element you may pass
    either a selector string or an element you captured earlier with `Query`:

    | Pointer Click | //Button[@Name="OK"] |
    | ${ok}=        | Query | //Button[@Name="OK"] | only_first=${True} |
    | Pointer Click | ${ok} |

    Unlike `Query`, these keywords wait for their element to appear before acting (see *Waiting for
    elements*).

    = Scoping queries with Set Root =

    `Set Root` sets a default context node, so you do not have to repeat a long
    ``//app:Application[...]//...`` prefix on every selector. Once it is set, every *relative*
    selector (``.//``, ``./``) searches inside that node — both in `Query` (when you do not pass an
    explicit ``root``) and in the elements the action keywords resolve. As with a per-call
    ``root``, an absolute ``//`` still starts at the desktop and ignores it.

    `Set Root` returns the previous root, so you can set one for a block of steps and put the old
    one back afterwards; ``Set Root    ${None}`` clears it and returns to the desktop. For a single
    query, pass ``root=`` to `Query` instead (see *Scoping a query to a parent*).

    | ${dialog}=    | Query | /Window[@Name="Settings"] | only_first=${True} |
    | ${previous}=  | Set Root | ${dialog} |
    | Pointer Click | .//Button[@Name="Apply"] |
    | Set Root      | ${previous} |

    == How the root is scoped ==

    The root is stored as a Robot Framework variable. This is deliberate: its lifetime then follows
    Robot Framework's ordinary variable scoping, so it behaves predictably and cleans up after
    itself.

    - It applies to the rest of the *current* test — a root set in a test's ``[Setup]`` or in its
      body covers that body.
    - It is *local*: it does not reach into the keywords you call. A called keyword keeps the
      default (or whatever root it sets for itself), so a caller's root never silently changes what
      a shared keyword does; a `Set Root` inside a keyword is local to that keyword.
    - It is cleared automatically when the test (or keyword) ends, so a root set in one test never
      leaks into the next — there is no teardown to remember.

    Because `Set Root` stores the *query*, not a fixed node, the root is re-resolved against the
    live tree whenever it is used — so it keeps working even if its window is closed and reopened.

    = Waiting for elements =

    Keywords that act on or read an element wait for it automatically: when the element is not on
    screen yet, the lookup keeps retrying for up to 30 seconds before failing the keyword, so you
    usually do not need explicit sleeps while the UI catches up.

    `Query` is the exception: it always reports the *current* state and never waits — it returns
    immediately, even when nothing matches yet.

    = Assertions =

    Keywords that read a value — such as `Get Attribute` and `Get Pointer Position` — can check it
    in the same call: add an assertion operator and the expected value. The keyword still returns
    the value, and fails if the assertion does not hold.

    | Get Attribute | //CheckBox[@Name="Dark mode"] | IsEnabled | == | ${True} |
    | Get Attribute | /Window | Name | contains | Settings |

    Common operators are ``==``, ``!=``, ``contains``, ``starts``, ``ends`` and ``matches`` — see
    [https://github.com/MarketSquare/AssertionEngine|AssertionEngine] for the full set.

    = A short example =

    | Pointer Click   | //Button[@Name="New"] |
    | Keyboard Type   | ${None} | Report 2024 |
    | ${name}=        | Get Attribute | /Window[@Name="Report 2024"] | Name |
    | Take Screenshot |
    """

    def __init__(
        self,
        *,
        keyboard_profile: KeyboardProfileLike | None = None,
        pointer_settings: PointerSettingsLike | None = None,
        pointer_profile: PointerProfileLike | None = None,
        use_mock: bool = False,
        auto_activate: bool = True,
    ) -> None:
        """Import the library.

        In the simplest case import it without arguments:

        | Library | PlatynUI.BareMetal |

        The optional settings configure default input behaviour or select a mock backend:

        | Library | PlatynUI.BareMetal | auto_activate=${False} |
        | Library | PlatynUI.BareMetal | use_mock=${True} |
        | Library | PlatynUI.BareMetal | pointer_profile=${POINTER_PROFILE} |

        Arguments:
        - ``keyboard_profile`` - default keyboard timing (a ``KeyboardProfile`` or a dict of its fields).
        - ``pointer_settings`` - default pointer settings (a ``PointerSettings`` or a dict).
        - ``pointer_profile`` - default pointer motion profile (a ``PointerProfile`` or a dict).
        - ``use_mock`` - use an in-process mock backend instead of the real desktop, for tests. Default ``False``.
        - ``auto_activate`` - bring an element's window to the front before pointer actions. Default ``True``.
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
    def set_root(self, descriptor: UiNodeDescriptor | None) -> UiNodeDescriptor | None:
        """Set the root UiNode for subsequent queries.

        Args:
            descriptor: A UiNodeDescriptor representing the node to set as the new root.
                        If None, resets to the default runtime context (desktop).

        Returns:
            UiNodeDescriptor | None: The previous root descriptor, or None if no root
            was set. Useful for saving and later restoring the root.

        Examples:
            | ${window}= | Query    | //control:Window[@Name="Settings"] | only_first=${True} |
            | ${old}=    | Set Root | ${window} |
            | ...        |
            | Set Root   | ${old}   |

        """
        old_root = (
            EXECUTION_CONTEXTS.current.variables[f'${{{PLATYNUI_ROOT_DESCRIPTOR}}}']  # pyright: ignore[reportOptionalMemberAccess]
            if PLATYNUI_ROOT_DESCRIPTOR in EXECUTION_CONTEXTS.current.variables  # pyright: ignore[reportOptionalMemberAccess]
            else None
        )

        EXECUTION_CONTEXTS.current.variables[f'${{{PLATYNUI_ROOT_DESCRIPTOR}}}'] = descriptor  # pyright: ignore[reportOptionalMemberAccess]

        return old_root

    @keyword
    def query(
        self,
        expression: str,
        root: UiNode | None = None,
        only_first: bool = False,
    ) -> Any:
        """Evaluate a PlatynUI XPath 2.0 expression against the live UI tree.

        Args:
            expression: XPath 2.0 selector/expression to evaluate. Examples:
                //control:Button[@Name="OK"], count(//control:Text).
            root: Optional evaluation root. If None, the runtime default
                context is used (e.g., desktop or current application).
            only_first: If True, return only the first match (or ``None`` when there is no match). If False,
                return all matches or the computed value of the expression.

        Returns:
            Any: When only_first is True, a single ``UiNode`` / value (or ``None`` if there is
            no match). Otherwise a list of nodes/values. Expressions that compute a value
            (e.g. ``count(...)``) return that value (int/float/str/bool) rather than nodes.
            Errors from the native runtime are propagated.

        Examples:
            | ${buttons}=    Query    //control:Button |
            | ${ok}=         Query    //control:Button[@Name="OK"]    only_first=${True} |
            | ${count}=      Query    count(//control:Button) |

        Notes:
            - Namespaces follow PlatynUI defaults (e.g., control); qualify names when needed.
            - Read-only: This keyword does not modify UI state.
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
        """Click at absolute or element-relative screen coordinates.

        Args:
            descriptor: Optional node to target. When provided:
                - If x/y are omitted: uses ActivationPoint if present, otherwise
                  the center of Bounds.
                - If x/y are given: they are offsets relative to the node's top-left Bounds.
            button: Mouse button to use. Defaults to LEFT.
            x: X coordinate. Absolute if no descriptor is provided; otherwise a relative offset.
            y: Y coordinate. Absolute if no descriptor is provided; otherwise a relative offset.
            activate: Whether to bring the element's window to the foreground before
                clicking. If None, the library-level ``auto_activate`` setting is used.

        Raises:
            ValueError: If only one of x or y is provided.

        Notes:
            With neither a descriptor nor x/y, the click happens at the current pointer position.

        Examples:
            | Pointer Click | //control:Button[@Name="OK"] |
            | Pointer Click | | x=${100} | y=${200} |
            | Pointer Click | //control:Button[@Name="OK"] | activate=${False} |
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
        """Perform multiple clicks at absolute or element-relative screen coordinates.

        Args:
            descriptor: Optional node to target. When provided:
                - If x/y are omitted: uses ActivationPoint if present, otherwise
                  the center of Bounds.
                - If x/y are given: they are offsets relative to the node's top-left Bounds.
            clicks: Number of clicks to perform. Defaults to 2 (double-click).
            button: Mouse button to use. Defaults to LEFT.
            x: X coordinate. Absolute if no descriptor is provided; otherwise a relative offset.
            y: Y coordinate. Absolute if no descriptor is provided; otherwise a relative offset.
            activate: Whether to bring the element's window to the foreground before
                clicking. If None, the library-level ``auto_activate`` setting is used.

        Raises:
            ValueError: If only one of x or y is provided.

        Notes:
            With neither a descriptor nor x/y, the clicks happen at the current pointer position.

        Examples:
            | Pointer Multi Click | //control:ListItem[@Name="Open"] |
            | Pointer Multi Click | | x=${100} | y=${200} |
            | Pointer Multi Click | //control:Text[@Name="File"] | clicks=${3} |
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
        """Press a mouse button at absolute or element-relative screen coordinates.

        Args:
            descriptor: Optional node to target. When provided:
                - If x/y are omitted: uses ActivationPoint if present, otherwise
                  the center of Bounds.
                - If x/y are given: they are offsets relative to the node's top-left Bounds.
            button: Mouse button to use. Defaults to LEFT.
            x: X coordinate. Absolute if no descriptor is provided; otherwise a relative offset.
            y: Y coordinate. Absolute if no descriptor is provided; otherwise a relative offset.
            activate: Whether to bring the element's window to the foreground before
                pressing. If None, the library-level ``auto_activate`` setting is used.

        Raises:
            ValueError: If only one of x or y is provided.

        Notes:
            With neither a descriptor nor x/y, the press happens at the current pointer position.

        Examples:
            | Pointer Press | //control:Slider | x=${10} | y=${5} |
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
        """Release a mouse button at current or specified coordinates.

        If a descriptor or coordinates are provided, the pointer is moved there first,
        then the button is released. Without a target, the button is released at the
        current pointer location.

        Args:
            descriptor: Optional node to target (see pointer_click for targeting rules).
            button: Mouse button to release. Defaults to LEFT.
            x: Optional X coordinate (see pointer_click for rules).
            y: Optional Y coordinate (see pointer_click for rules).
            activate: Whether to bring the element's window to the foreground before
                releasing. If None, the library-level ``auto_activate`` setting is used.

        Raises:
            ValueError: If only one of x or y is provided when targeting a location.

        Examples:
            | Pointer Release | | |
            | Pointer Release | //control:Canvas | x=${50} | y=${50} |
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
        """Move the pointer to absolute or element-relative screen coordinates.

        Args:
            descriptor: Optional node to target. When provided:
                - If x/y are omitted: uses ActivationPoint if present, otherwise
                  the center of Bounds.
                - If x/y are given: they are offsets relative to the node's top-left Bounds.
            x: X coordinate. Absolute if no descriptor is provided; otherwise a relative offset.
            y: Y coordinate. Absolute if no descriptor is provided; otherwise a relative offset.
            activate: Whether to bring the element's window to the foreground before
                moving. If None, the library-level ``auto_activate`` setting is used.

        Raises:
            ValueError: If only one of x or y is provided; or if neither coordinates nor a
            resolvable descriptor location are available.

        Examples:
            | Pointer Move To | | x=${400} | y=${300} |
            | Pointer Move To | //control:Button[@Name="OK"] |
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
        """Set input focus to the specified element.

        The target element is brought to the front (via the runtime) and focused using
        the platform's focus APIs. Use this before typing if an element isn't already
        focused.

        Args:
            descriptor: Element to focus. Can be a UiNode or a selector string.

        Examples:
            | Focus | //control:Edit[@Name="Search"] |
        """
        self.runtime.focus(descriptor())

    @keyword
    def restore_window(self, descriptor: UiNodeDescriptor) -> None:
        """Restore a window from minimized or maximized state.

        Operates through the element's ``Restorable`` pattern.
        Raises ``PatternError`` if the element doesn't support ``Restorable``.

        Args:
            descriptor: The window element to restore.

        Examples:
            | Restore Window | //control:Window[@Name="Settings"] |
        """
        node = descriptor()
        node.get_pattern(Restorable).restore()

    @keyword
    def maximize_window(self, descriptor: UiNodeDescriptor) -> None:
        """Maximize a window.

        Operates through the element's ``Maximizable`` pattern.
        Raises ``PatternError`` if the element doesn't support ``Maximizable``.

        Args:
            descriptor: The window element to maximize.

        Examples:
            | Maximize Window | //control:Window[@Name="Editor"] |
        """
        node = descriptor()
        node.get_pattern(Maximizable).maximize()

    @keyword
    def minimize_window(self, descriptor: UiNodeDescriptor) -> None:
        """Minimize a window.

        Operates through the element's ``Minimizable`` pattern.
        Raises ``PatternError`` if the element doesn't support ``Minimizable``.

        Args:
            descriptor: The window element to minimize.

        Examples:
            | Minimize Window | //control:Window[@Name="Editor"] |
        """
        node = descriptor()
        node.get_pattern(Minimizable).minimize()

    @keyword
    def close_window(self, descriptor: UiNodeDescriptor) -> None:
        """Close a window.

        Operates through the element's ``Closeable`` pattern.
        Raises ``PatternError`` if the element doesn't support ``Closeable``.

        Args:
            descriptor: The window element to close.

        Examples:
            | Close Window | //control:Window[@Name="Editor"] |
        """
        node = descriptor()
        node.get_pattern(Closeable).close()

    @keyword
    def activate_window(self, descriptor: UiNodeDescriptor) -> None:
        """Activate (bring to front and focus) a window.

        Operates through the element's ``Activatable`` pattern.
        Raises ``PatternError`` if the element doesn't support ``Activatable``.

        Args:
            descriptor: The window element to activate.

        Examples:
            | Activate Window | //control:Window[@Name="Editor"] |
        """
        node = descriptor()
        node.get_pattern(Activatable).activate()

    @keyword
    def move_window(self, descriptor: UiNodeDescriptor, x: float, y: float) -> None:
        """Move a window to the specified screen coordinates.

        Operates through the element's ``Movable`` pattern.
        Raises ``PatternError`` if the element doesn't support ``Movable``.

        Args:
            descriptor: The window element to move.
            x: The target x coordinate for the window's top-left corner.
            y: The target y coordinate for the window's top-left corner.

        Examples:
            | Move Window | //control:Window[@Name="Editor"] | 100 | 200 |
        """
        node = descriptor()
        node.get_pattern(Movable).move_to(x, y)

    @keyword
    def resize_window(self, descriptor: UiNodeDescriptor, width: float, height: float) -> None:
        """Resize a window to the specified dimensions.

        Operates through the element's ``Resizable`` pattern.
        Raises ``PatternError`` if the element doesn't support ``Resizable``.

        Args:
            descriptor: The window element to resize.
            width: The target width.
            height: The target height.

        Examples:
            | Resize Window | //control:Window[@Name="Editor"] | 800 | 600 |
        """
        node = descriptor()
        node.get_pattern(Resizable).resize(width, height)

    @keyword
    def move_and_resize_window(
        self, descriptor: UiNodeDescriptor, x: float, y: float, width: float, height: float
    ) -> None:
        """Move and resize a window in a single operation.

        Composes the element's ``Movable`` and ``Resizable`` patterns.
        Raises ``PatternError`` if the element doesn't support either pattern.

        Args:
            descriptor: The window element to move and resize.
            x: The target x coordinate for the window's top-left corner.
            y: The target y coordinate for the window's top-left corner.
            width: The target width.
            height: The target height.

        Examples:
            | Move And Resize Window | //control:Window[@Name="Editor"] | 100 | 200 | 800 | 600 |
        """
        node = descriptor()
        node.get_pattern(Movable).move_to(x, y)
        node.get_pattern(Resizable).resize(width, height)

    @keyword
    def bring_to_front(self, descriptor: UiNodeDescriptor) -> None:
        """Bring the specified UiNode to the front.

        Args:
            descriptor: The UiNodeDescriptor representing the target node.
        """
        node = descriptor()
        self.runtime.bring_to_front(node)

    @keyword
    @assertable
    def get_attribute(self, descriptor: UiNodeDescriptor, attribute_name: str) -> Any:
        """Get an attribute value from the specified UiNode.

        Args:
            descriptor: The UiNodeDescriptor representing the target node.
            attribute_name: The name of the attribute to retrieve.

        Returns:
            Any: The value of the specified attribute.

        This keyword is assertable: pass ``assertion_operator`` (and ``assertion_expected``)
        to verify the value, e.g. ``| Get Attribute | //control:Edit | Text | == | hello |``.
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
            overrides: Optional per-call keyboard timing overrides.

        Examples:
            | Keyboard Type | //control:Edit[@Name="Search"] | Hello World |
            | Keyboard Type | //control:Edit[@Name="Search"] | <Ctrl+A><Delete> |
            | Keyboard Type | ${None} | Hello\nWorld |  # newline supported

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
            overrides: Optional per-call keyboard timing overrides.

        Examples:
            | Keyboard Press   | //control:Window[@Name="Terminal"] | <Ctrl+Alt+T> |
            | Keyboard Press   | ${None} | <Ctrl> |
            | Keyboard Release | ${None} | <Ctrl> |
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
            overrides: Optional per-call keyboard timing overrides.

        Examples:
            | Keyboard Press   | //control:Window[@Name="Terminal"] | <Ctrl+Alt> |
            | Keyboard Release | //control:Window[@Name="Terminal"] | <Ctrl+Alt> |
            | Keyboard Release | ${None} | <Ctrl+Alt> |
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
            | Take Screenshot | | filename=EMBED |
            | Take Screenshot | | filename=full_desktop.png |
            | Take Screenshot | //control:Window[@Name="Settings"] | filename=settings_window.png |
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
        """Highlight a UI element for a specified duration.

        Args:
            descriptor: The UiNodeDescriptor(s) whose bounds to highlight. Takes precedence
                over ``rect`` when given.
            rect: Optional Rect(s) to highlight directly; used only when ``descriptor`` is None.
            duration: Duration in seconds (converted to milliseconds for the runtime).
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
