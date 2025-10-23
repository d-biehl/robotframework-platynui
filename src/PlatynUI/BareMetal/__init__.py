import base64
from functools import cached_property
from pathlib import Path
from typing import Any, Literal, cast

from platynui_native import Point, PointerButton, Rect, Runtime, UiNode, WindowSurface
from robot.api import logger
from robot.api.deco import library
from robot.libraries.BuiltIn import BuiltIn

from ..__version__ import __version__
from .._assertable import assertable
from .._our_libcore import OurDynamicCore, keyword


class UiNodeDescriptor:
    """Descriptor wrapper allowing lazy resolution of a UiNode from a query string.

    The descriptor either holds a concrete UiNode or an expression string that will be
    evaluated using the associated BareMetal library instance when called.
    """

    def __init__(self, node: UiNode | str, library: 'BareMetal') -> None:
        self.node = node
        self.library = library

    def __call__(self) -> UiNode:
        if isinstance(self.node, UiNode):
            return self.node

        result = self.library.query(self.node, only_first=True)
        if result is None:
            raise ValueError(f'Query for UiNodeDescriptor {self.node!r} returned no results')

        if not isinstance(result, UiNode):
            raise TypeError(f'Query for UiNodeDescriptor {self.node!r} did not return a UiNode, got: {result!r}')

        self.node = result  # Cache resolved node

        return result

    @staticmethod
    def convert(value: str | UiNode, library: 'BareMetal') -> 'UiNodeDescriptor':
        return UiNodeDescriptor(value, library)


@library(
    scope='GLOBAL', version=__version__, converters={UiNodeDescriptor: UiNodeDescriptor.convert}, doc_format='ROBOT'
)
class BareMetal(OurDynamicCore):
    """Robot Framework library for PlatynUI's native backend.

    This library exposes low-level, platform-aware UI automation keywords backed by the
    PlatynUI runtime. It allows you to query the UI tree (XPath-like expressions),
    perform pointer and keyboard actions, and operate on window surfaces.

    """

    def __init__(self) -> None:
        super().__init__([])
        self._screenshot_counter = 1

    @cached_property
    def runtime(self) -> Runtime:
        """Return the PlatynUI BareMetal runtime instance.

        The runtime bridges this Robot Framework library with the native PlatynUI engine,
        enabling XPath-like queries and actions against the UI tree.
        """
        return Runtime()

    @cached_property
    def _screenshot_path(self) -> Path:
        return Path(str(BuiltIn().get_variable_value('${OUTPUT DIR}', default='screenshots')))  # pyright: ignore[reportUnknownArgumentType]

    @keyword
    def query(
        self,
        expression: str,
        root: Any = None,
        only_first: bool = False,
    ) -> Any:
        """Evaluate a PlatynUI XPath-like expression against the UI tree.

        Args:
            expression: XPath-like selector/expression to evaluate. Examples:
                //control:Button[@Name="OK"], count(//control:Text).
            root: Optional evaluation root. If None, the runtime default
                context is used (e.g., desktop or current application).
            only_first: If True, return only the first match (or ``None`` when there is no match). If False,
                return all matches or the computed value of the expression.

        Returns:
            Any: A single node/value when only_first is True; otherwise a list/sequence
            of nodes/values as produced by the runtime. Errors from the native runtime
            are propagated.

        Examples:
            | ${buttons}=    Query    //control:Button |
            | ${ok}=         Query    //control:Button[@Name="OK"]    only_first=${True} |
            | ${count}=      Query    count(//control:Button) |

        Notes:
            - Namespaces follow PlatynUI defaults (e.g., control); qualify names when needed.
            - Read-only: This keyword does not modify UI state.
        """
        result = (
            self.runtime.evaluate_single(expression, root) if only_first else self.runtime.evaluate(expression, root)
        )

        return result

    # Internal helpers
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
        - If neither descriptor nor x/y are provided: raises ValueError.

        Returns:
        - Point: Absolute screen coordinates to use for pointer actions.
        """
        if (x is not None) != (y is not None):
            raise ValueError('Both x and y coordinates must be provided together')

        if descriptor is not None:
            target_node = descriptor()

            self.runtime.bring_to_front(target_node)

            # No coordinates provided: auto-resolve from node
            if x is None and y is None:
                activation_point = target_node.attribute('ActivationPoint')
                if isinstance(activation_point, Point):
                    x = activation_point.x
                    y = activation_point.y
                else:
                    bounds = target_node.attribute('Bounds')
                    if not isinstance(bounds, Rect):
                        raise ValueError('Node has no bounds or activation point')

                    # Keep integer-style center as before for backward compatibility
                    x = bounds.x + bounds.width // 2
                    y = bounds.y + bounds.height // 2

            # Relative coordinates provided: offset from node bounds
            elif x is not None and y is not None:
                bounds = target_node.attribute('Bounds')
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
        button: PointerButton | int = PointerButton.LEFT,
        x: float | None = None,
        y: float | None = None,
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

        Raises:
            ValueError: If only one of x or y is provided; or if neither coordinates nor a
            resolvable descriptor location are available.

        Examples:
            | Pointer Click | //control:Button[@Name="OK"] |
            | Pointer Click | | x=${100} | y=${200} |
        """
        point = self._resolve_screen_point(descriptor, x, y)
        self.runtime.pointer_click(point, button)

    @keyword
    def pointer_multi_click(
        self,
        descriptor: UiNodeDescriptor | None = None,
        *,
        clicks: int = 2,
        button: PointerButton | int = PointerButton.LEFT,
        x: float | None = None,
        y: float | None = None,
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

        Raises:
            ValueError: If only one of x or y is provided; or if neither coordinates nor a
            resolvable descriptor location are available.

        Examples:
            | Pointer Multi Click | //control:ListItem[@Name="Open"] |
            | Pointer Multi Click | | x=${100} | y=${200} |
            | Pointer Multi Click | //control:Text[@Name="File"] | clicks=${3} |
        """
        point = self._resolve_screen_point(descriptor, x, y)
        self.runtime.pointer_multi_click(point, clicks, button)

    @keyword
    def pointer_press(
        self,
        descriptor: UiNodeDescriptor | None = None,
        *,
        button: PointerButton | int = PointerButton.LEFT,
        x: float | None = None,
        y: float | None = None,
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

        Raises:
            ValueError: If only one of x or y is provided; or if neither coordinates nor a
            resolvable descriptor location are available.

        Examples:
            | Pointer Press | //control:Slider | x=${10} | y=${5} |
        """
        point = self._resolve_screen_point(descriptor, x, y)
        self.runtime.pointer_press(point, button)

    @keyword
    def pointer_release(
        self,
        descriptor: UiNodeDescriptor | None = None,
        *,
        button: PointerButton | int = PointerButton.LEFT,
        x: float | None = None,
        y: float | None = None,
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

        Raises:
            ValueError: If only one of x or y is provided when targeting a location.

        Examples:
            | Pointer Release | | |
            | Pointer Release | //control:Canvas | x=${50} | y=${50} |
        """
        point = self._resolve_screen_point(descriptor, x, y)
        self.runtime.pointer_release(point, button)

    @keyword
    def pointer_move_to(
        self,
        descriptor: UiNodeDescriptor | None = None,
        *,
        x: float | None = None,
        y: float | None = None,
    ) -> None:
        """Move the pointer to absolute or element-relative screen coordinates.

        Args:
            descriptor: Optional node to target. When provided:
                - If x/y are omitted: uses ActivationPoint if present, otherwise
                  the center of Bounds.
                - If x/y are given: they are offsets relative to the node's top-left Bounds.
            x: X coordinate. Absolute if no descriptor is provided; otherwise a relative offset.
            y: Y coordinate. Absolute if no descriptor is provided; otherwise a relative offset.

        Raises:
            ValueError: If only one of x or y is provided; or if neither coordinates nor a
            resolvable descriptor location are available.

        Examples:
            | Pointer Move To | | x=${400} | y=${300} |
            | Pointer Move To | //control:Button[@Name="OK"] |
        """
        point = self._resolve_screen_point(descriptor, x, y)
        if point is None:
            raise ValueError('Coordinates x and y must be specified either directly or via node')

        self.runtime.pointer_move_to(point)

    @keyword
    @assertable
    def get_pointer_position(self) -> Any:
        """Get the current pointer position on the screen.

        Returns:
            Point: The current screen coordinates of the pointer.
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
    def restore(self, descriptor: UiNodeDescriptor) -> None:
        """Restore a window from minimized or maximized state.

        Operates through the element's ``WindowSurface`` pattern when available.
        If the element doesn't support ``WindowSurface``, this is a no-op.

        Args:
            descriptor: The window element to restore.

        Examples:
            | Restore | //control:Window[@Name="Settings"] |
        """
        node = descriptor()
        pattern = node.pattern_by_id('WindowSurface')
        if isinstance(pattern, WindowSurface):
            pattern.restore()

    @keyword
    def maximize(self, descriptor: UiNodeDescriptor) -> None:
        """Maximize a window.

        Uses the element's ``WindowSurface`` pattern if supported. No-op otherwise.

        Args:
            descriptor: The window element to maximize.

        Examples:
            | Maximize | //control:Window[@Name="Editor"] |
        """
        node = descriptor()
        pattern = node.pattern_by_id('WindowSurface')
        if isinstance(pattern, WindowSurface):
            pattern.maximize()

    @keyword
    def minimize(self, descriptor: UiNodeDescriptor) -> None:
        """Minimize a window.

        Uses the element's ``WindowSurface`` pattern if supported. No-op otherwise.

        Args:
            descriptor: The window element to minimize.

        Examples:
            | Minimize | //control:Window[@Name="Editor"] |
        """
        node = descriptor()
        pattern = node.pattern_by_id('WindowSurface')
        if isinstance(pattern, WindowSurface):
            pattern.minimize()

    @keyword
    def close(self, descriptor: UiNodeDescriptor) -> None:
        """Close a window.

        Uses the element's ``WindowSurface`` pattern if supported. No-op otherwise.

        Args:
            descriptor: The window element to close.

        Examples:
            | Close | //control:Window[@Name="Editor"] |
        """
        node = descriptor()
        pattern = node.pattern_by_id('WindowSurface')
        if isinstance(pattern, WindowSurface):
            pattern.close()

    @keyword
    def activate(self, descriptor: UiNodeDescriptor) -> None:
        """Activate (bring to front and focus) a window.

        Uses the element's ``WindowSurface`` pattern if supported. No-op otherwise.

        Args:
            descriptor: The window element to activate.

        Examples:
            | Activate | //control:Window[@Name="Editor"] |
        """
        node = descriptor()
        pattern = node.pattern_by_id('WindowSurface')
        if isinstance(pattern, WindowSurface):
            pattern.activate()

    # TODO: Add more keywords for other interactions and patterns as needed

    @keyword
    @assertable
    def get_attribute(self, descriptor: UiNodeDescriptor, attribute_name: str) -> Any:
        """Get an attribute value from the specified UiNode.

        Args:
            descriptor: The UiNodeDescriptor representing the target node.
            attribute_name: The name of the attribute to retrieve.

        Returns:
            Any: The value of the specified attribute.
        """
        namespace: str | None = None
        if ':' in attribute_name:
            namespace, attribute_name = attribute_name.split(':', 1)
        node = descriptor()
        return node.attribute(attribute_name, namespace)

    @keyword
    def keyboard_type(
        self,
        descriptor: UiNodeDescriptor | None,
        text: str,
    ) -> None:
        r"""Type a sequence of characters and/or keys.

        If ``descriptor`` is provided, the element is brought to front and focused first.
        Sequences may include plain text and special keys wrapped in angle brackets.
        Use ``+`` to combine modifiers with keys.

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
            self.runtime.bring_to_front(target_node)
            self.runtime.focus(target_node)
        self.runtime.keyboard_type(text)

    @keyword
    def keyboard_press(
        self,
        descriptor: UiNodeDescriptor | None,
        text: str,
    ) -> None:
        """Press (and hold) keys according to a sequence.

        Unlike ``Keyboard Type``, this sends only press events (no release). Use this to
        hold modifiers or keys; pair with ``Keyboard Release`` to complete the action.

        Args:
            descriptor: Optional element to bring to front and focus before pressing.
            text: Sequence of keys, e.g. ``<Ctrl+Alt+T>`` or ``<Shift>``.

        Examples:
            | Keyboard Press   | //control:Window[@Name="Terminal"] | <Ctrl+Alt+T> |
            | Keyboard Press   | ${None} | <Ctrl> |
            | Keyboard Release | ${None} | <Ctrl> |
        """
        if descriptor is not None:
            target_node = descriptor()
            self.runtime.bring_to_front(target_node)
            self.runtime.focus(target_node)
        self.runtime.keyboard_press(text)

    @keyword
    def keyboard_release(
        self,
        descriptor: UiNodeDescriptor | None,
        text: str,
    ) -> None:
        """Release keys according to a sequence.

        Complements ``Keyboard Press`` by releasing keys/modifiers. If you need a full
        press→release cycle for characters or shortcuts, prefer ``Keyboard Type``.

        Args:
            descriptor: Optional element to bring to front and focus before releasing.
            text: Sequence of keys to release, e.g. ``<Ctrl+Alt+T>`` or ``<Ctrl>``.

        Examples:
            | Keyboard Press   | //control:Window[@Name="Terminal"] | <Ctrl+Alt> |
            | Keyboard Release | //control:Window[@Name="Terminal"] | <Ctrl+Alt> |
            | Keyboard Release | ${None} | <Ctrl+Alt> |
        """
        if descriptor is not None:
            target_node = descriptor()
            self.runtime.bring_to_front(target_node)
            self.runtime.focus(target_node)
        self.runtime.keyboard_release(text)

    @keyword
    def take_screenshot(
        self,
        descriptor: UiNodeDescriptor | None = None,
        filename: Literal['EMBED'] | str = 'platynui-screenshot-{index}.png',
        rect: Rect | None = None,
    ) -> str:
        """Take a screenshot of the entire screen or a specific element.

        Args:
            descriptor: Optional element to capture. If None, captures the full screen.
            filename: Literal['EMBED'] or path to save the screenshot image.
            rect: Optional rectangle area to capture. If provided, captures this area

        Examples:
            | Take Screenshot | | file_path=screenshots/full_desktop.png |
            | Take Screenshot | //control:Window[@Name="Settings"] | file_path=screenshots/settings_window.png |
        """
        if descriptor is not None:
            node = descriptor()
            self.runtime.bring_to_front(node)
            rect = cast(Rect, node.attribute('Bounds'))

        screenshot = self.runtime.screenshot(rect, 'image/png')
        if filename == 'EMBED':
            logger.info(
                '</td></tr><tr><td colspan="3">'
                '<img alt="screenshot" class="robot-seleniumlibrary-screenshot" '
                f'src="data:image/png;base64,{base64.b64encode(screenshot).decode("utf-8")}" width="900px"/>',
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
            f'<a href="{relative_path}" target="_blank"><img src="{relative_path}" width="800px"/></a>',
            html=True,
        )

        return filename
