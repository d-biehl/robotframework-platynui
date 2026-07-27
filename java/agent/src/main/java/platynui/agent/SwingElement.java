package platynui.agent;

import java.awt.Component;
import java.awt.Container;
import java.awt.Frame;
import java.awt.Rectangle;
import java.awt.Window;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import javax.accessibility.Accessible;
import javax.accessibility.AccessibleContext;
import javax.accessibility.AccessibleExtendedText;
import javax.accessibility.AccessibleRole;
import javax.accessibility.AccessibleSelection;
import javax.accessibility.AccessibleState;
import javax.accessibility.AccessibleStateSet;
import javax.accessibility.AccessibleTable;
import javax.accessibility.AccessibleText;
import javax.accessibility.AccessibleValue;
import javax.swing.JComponent;
import javax.swing.JTable;
import javax.swing.table.JTableHeader;
import javax.swing.table.TableColumn;
import javax.swing.table.TableColumnModel;
import javax.swing.text.JTextComponent;

/**
 * One element as it travels the wire: the toolkit's own view <em>and</em> the accessible view of the
 * same object, in one message (design decision 1).
 *
 * <p>Both levels, not one, and that is the whole point of reading from inside the process. The
 * <strong>instance tree</strong> is where {@link Component#getName()} lives — the classic automation
 * id no out-of-process bridge can see — along with the real geometry, the model, and the semantic
 * actions. The <strong>accessible view</strong> is what existing locators are written against, so it
 * travels alongside instead of being replaced: an app that only ever annotated accessibility stays
 * addressable, and a JAB-era locator on {@code accessibleName} keeps matching.
 *
 * <p>Coarse-grained on purpose: a node's whole surface arrives in one frame, and a table's model
 * comes in bulk. The wire is affordable <em>because</em> it is coarse — the alternative, a call per
 * property, is what makes out-of-process accessibility slow.
 *
 * <p>Roles and states are emitted as their {@code Locale.ENGLISH} display strings, which are exactly
 * the {@code role_en_US}/{@code states_en_US} vocabularies the Java Access Bridge reports. That is
 * deliberate: the provider maps one vocabulary, and the same Swing application answers the same
 * selectors whichever backend serves it.
 */
final class SwingElement {

    /** What kind of thing this element is; decides which of the optional blocks are present. */
    static final String KIND_WINDOW = "window";

    static final String KIND_COMPONENT = "component";
    static final String KIND_CELL = "cell";
    static final String KIND_ACCESSIBLE = "accessible";

    /**
     * Upper bound for the selected-children scan, so one attribute read against a fully selected
     * list cannot register tens of thousands of elements.
     */
    private static final int SELECTION_SCAN_LIMIT = 512;

    private SwingElement() {
        // Static helper.
    }

    /**
     * Builds the payload for {@code element}, which is either a {@link Component} or a
     * {@link SwingTree.VirtualChild}.
     *
     * <p>Must run on the toolkit thread: every read below is a toolkit question, and asking one off
     * the event thread is how an observer causes the races it then reports as flakiness.
     */
    static Map<String, Object> describe(Object element, ElementRegistry registry, int childCount) {
        Map<String, Object> payload = Json.newObject();
        payload.put("id", Long.valueOf(registry.idFor(element)));
        if (element instanceof SwingTree.VirtualChild) {
            describeVirtual((SwingTree.VirtualChild) element, payload);
        } else if (element instanceof Component) {
            describeComponent((Component) element, payload, registry);
        } else {
            payload.put("kind", KIND_ACCESSIBLE);
            payload.put("className", element.getClass().getName());
        }
        payload.put("childCount", Long.valueOf(childCount));
        return payload;
    }

    // ------------------------------------------------------------- components

    private static void describeComponent(Component component, Map<String, Object> payload, ElementRegistry registry) {
        boolean isWindow = component instanceof Window;
        payload.put("kind", isWindow ? KIND_WINDOW : KIND_COMPONENT);
        payload.put("className", component.getClass().getName());

        AccessibleContext context = accessibleContextOf(component);
        payload.put("role", roleOf(context, component, isWindow));
        // `Component.getName()` is the spine's exclusive contribution — but only
        // when the developer actually set it. See {@link #explicitNameOf}.
        putIfPresent(payload, "name", explicitNameOf(component));
        if (context != null) {
            putIfPresent(payload, "accessibleName", context.getAccessibleName());
            putIfPresent(payload, "accessibleDescription", context.getAccessibleDescription());
            payload.put("states", statesOf(context));
        } else {
            payload.put("states", new ArrayList<Object>());
        }

        payload.put("enabled", Boolean.valueOf(component.isEnabled()));
        payload.put("visible", Boolean.valueOf(component.isVisible()));
        payload.put("showing", Boolean.valueOf(component.isShowing()));
        payload.put("focusable", Boolean.valueOf(component.isFocusable()));
        payload.put("focused", Boolean.valueOf(component.isFocusOwner()));

        Map<String, Object> bounds = SwingGeometry.boundsOf(component);
        if (bounds != null) {
            payload.put("bounds", bounds);
        }

        // Present whenever the element *can* hold text, even when it currently holds
        // none. The distinction matters: consumers derive the TextContent capability
        // from this field's presence, so dropping an empty value would make an empty
        // text field indistinguishable from a label that has no text at all.
        String text = textOf(component, context);
        if (text != null) {
            payload.put("text", text);
        }
        Boolean editable = editableOf(component, context);
        if (editable != null) {
            payload.put("editable", editable);
        }
        putIfPresent(payload, "toolTipText", toolTipOf(component));

        Map<String, Object> value = valueOf(context);
        if (value != null) {
            payload.put("value", value);
        }
        Map<String, Object> selection = selectionOf(context, component, registry);
        if (selection != null) {
            payload.put("selection", selection);
        }
        Map<String, Object> table = tableOf(component, context);
        if (table != null) {
            payload.put("table", table);
        }
        Map<String, Object> clientProperties = SwingClientProperties.of(component);
        if (clientProperties != null && !clientProperties.isEmpty()) {
            payload.put("clientProperties", clientProperties);
        }
        if (isWindow) {
            payload.put("window", windowFactsOf((Window) component));
        }
    }

    /** The window-only block: what the provider needs to delegate the window patterns. */
    private static Map<String, Object> windowFactsOf(Window window) {
        Map<String, Object> facts = Json.newObject();
        SwingWindowHandle.describeInto(facts, window);
        String title = window instanceof Frame ? ((Frame) window).getTitle() : null;
        if (title == null && window instanceof java.awt.Dialog) {
            title = ((java.awt.Dialog) window).getTitle();
        }
        putIfPresent(facts, "title", title);
        facts.put("active", Boolean.valueOf(window.isActive()));
        facts.put("focused", Boolean.valueOf(window.isFocused()));
        if (window instanceof Frame) {
            Frame frame = (Frame) window;
            facts.put("resizable", Boolean.valueOf(frame.isResizable()));
            // `Frame.getExtendedState()` is the only place the maximized/iconified
            // distinction is available without asking the platform.
            facts.put("extendedState", Long.valueOf(frame.getExtendedState()));
        }
        facts.put("alwaysOnTop", Boolean.valueOf(window.isAlwaysOnTop()));
        return facts;
    }

    // ---------------------------------------------------------- virtual cells

    /**
     * A child the toolkit has no component for: a table cell, a list entry, a tree row.
     *
     * <p>For a table cell the authoritative name comes from the <strong>model</strong>, not from the
     * accessible wrapper. Both are reported, and the difference is the entire reason this change
     * exists: the JDK aliases every {@code JTable} cell to one shared renderer component, so the
     * accessible view of a cell is only correct while that renderer happens to be configured for it
     * — a condition an out-of-process bridge cannot hold, and the source of the volatile cell names
     * JAB reports. The model read has no such window of validity.
     */
    private static void describeVirtual(SwingTree.VirtualChild child, Map<String, Object> payload) {
        Component owner = child.owner();
        boolean isCell = child.isCell();
        payload.put("kind", isCell ? KIND_CELL : KIND_ACCESSIBLE);
        payload.put("className", owner == null ? "" : owner.getClass().getName());

        AccessibleContext wrapper = child.accessibleContext();
        payload.put("role", wrapper == null ? "unknown" : displayString(wrapper.getAccessibleRole()));
        if (wrapper != null) {
            putIfPresent(payload, "accessibleName", wrapper.getAccessibleName());
            putIfPresent(payload, "accessibleDescription", wrapper.getAccessibleDescription());
            payload.put("states", statesOf(wrapper));
        } else {
            payload.put("states", new ArrayList<Object>());
        }

        if (isCell && owner instanceof JTable) {
            JTable table = (JTable) owner;
            int row = child.row();
            int column = child.column();
            payload.put("name", modelValueAt(table, row, column));
            // Only the part that is on screen: a table larger than its viewport
            // still answers `getCellRect` for every cell of the model, and a
            // rectangle below the window is worse than none (see
            // `SwingTree.visiblePart`).
            Rectangle onScreen = SwingTree.visiblePart(table, table.getCellRect(row, column, false));
            Map<String, Object> bounds = SwingGeometry.boundsWithin(table, onScreen);
            if (bounds != null) {
                payload.put("bounds", bounds);
            }
            Map<String, Object> cell = Json.newObject();
            cell.put("row", Long.valueOf(row));
            cell.put("column", Long.valueOf(column));
            // Swing's own tables never span; the extents travel anyway so the
            // provider's attribute surface matches the bridge's, which reports
            // them for tables that do.
            cell.put("rowExtent", Long.valueOf(1L));
            cell.put("columnExtent", Long.valueOf(1L));
            cell.put("selected", Boolean.valueOf(table.isCellSelected(row, column)));
            cell.put("editable", Boolean.valueOf(table.isCellEditable(row, column)));
            payload.put("cell", cell);
            payload.put("enabled", Boolean.valueOf(table.isEnabled()));
            payload.put("visible", Boolean.valueOf(table.isVisible()));
            // `showing` is "in view", and a scrolled-out cell is not — which is
            // a different question from `visible`, i.e. "not hidden by its own
            // flag". The table answers the second for it; only the viewport can
            // answer the first.
            payload.put("showing", Boolean.valueOf(table.isShowing() && onScreen != null));
        } else if (child.isRow() && owner instanceof JTable) {
            describeTableRow((JTable) owner, child.row(), payload);
        } else if (owner instanceof JTableHeader) {
            describeColumnHeader((JTableHeader) owner, child.index(), payload, wrapper);
        } else {
            // A non-cell virtual child (list entry, tree row): the accessible
            // wrapper is the only view there is, and unlike a table cell it is not
            // renderer-aliased per lookup.
            if (wrapper != null) {
                putIfPresent(payload, "name", wrapper.getAccessibleName());
            }
            Map<String, Object> bounds = virtualBounds(owner, wrapper);
            if (bounds != null) {
                payload.put("bounds", bounds);
            }
            payload.put("enabled", Boolean.valueOf(owner != null && owner.isEnabled()));
            payload.put("visible", Boolean.valueOf(owner != null && owner.isVisible()));
            payload.put("showing", Boolean.valueOf(owner != null && owner.isShowing()));
        }
        payload.put("focusable", Boolean.FALSE);
        payload.put("focused", Boolean.FALSE);
    }

    /**
     * A table row — a level the accessible view of a {@code JTable} does not have at all.
     *
     * <p>Swing's accessible projection of a table is a flat, row-major list of cells, which is why
     * the Access Bridge has no rows and why this level can only come from the model. Everything
     * about the row is read there: its extent is the union of its cells' rectangles, its selection
     * is the table's row selection.
     *
     * <p><strong>No name.</strong> A row carries no label of its own, and synthesising one by
     * joining its cells' values would invent an identifier that changes whenever any cell does. A
     * row is addressed by position or by what it contains — which is how the rows of every other
     * provider behave too.
     */
    private static void describeTableRow(JTable table, int row, Map<String, Object> payload) {
        // Override the wrapper's role — there is no wrapper for a row — with what
        // this element is; the provider maps `table row` to `item:TableRow`.
        payload.put("role", "table row");

        // Clipped to the viewport, like its cells: most rows of a scrolling
        // table are nowhere on screen, and a rectangle below the window is
        // worse than none (see `SwingTree.visiblePart`).
        Rectangle onScreen = SwingTree.visiblePart(table, SwingTree.rowRect(table, row));
        Map<String, Object> bounds = SwingGeometry.boundsWithin(table, onScreen);
        if (bounds != null) {
            payload.put("bounds", bounds);
        }

        boolean selected = table.isRowSelected(row);
        Map<String, Object> block = Json.newObject();
        block.put("row", Long.valueOf(row));
        block.put("selected", Boolean.valueOf(selected));
        payload.put("tableRow", block);

        // Derived states rather than the wrapper's, for the same reason as the
        // role: the accessible view has nothing to say about a row. They travel on
        // the shared vocabulary, so the provider's normalised `Selectable` surface
        // resolves without knowing that this element came from a table.
        List<Object> states = new ArrayList<Object>();
        if (table.getRowSelectionAllowed()) {
            states.add("selectable");
            if (selected) {
                states.add("selected");
            }
        }
        payload.put("states", states);

        payload.put("enabled", Boolean.valueOf(table.isEnabled()));
        payload.put("visible", Boolean.valueOf(table.isVisible()));
        payload.put("showing", Boolean.valueOf(table.isShowing() && onScreen != null));
    }

    /**
     * A column header, read from the header component rather than from its accessible wrapper.
     *
     * <p>The same argument as for cells, and the same defect underneath. Swing's
     * {@code AccessibleJTableHeaderEntry} answers the renderer's role — {@code label} — and a
     * <strong>zero-height</strong> rectangle, because the renderer it delegates to was never laid
     * out. So a column header arrives as a label with no place on screen, although it is one of the
     * most clickable things in a table: sorting, resizing and reordering all happen there.
     *
     * <p>The header component knows better. {@code getHeaderRect} is the rectangle the user sees, and
     * the column model carries the header's value and its model index — the latter being the one that
     * survives the user dragging columns around, which is exactly what a test wants to address.
     */
    private static void describeColumnHeader(
            JTableHeader header, int column, Map<String, Object> payload, AccessibleContext wrapper) {
        TableColumnModel columns = header.getColumnModel();
        boolean valid = column >= 0 && column < columns.getColumnCount();

        // Override the renderer's role with what this element actually is; the
        // provider maps `column header` to `item:ColumnHeader`.
        payload.put("role", "column header");
        if (valid) {
            TableColumn model = columns.getColumn(column);
            Object value = model.getHeaderValue();
            if (value != null) {
                payload.put("name", String.valueOf(value));
            } else if (wrapper != null) {
                putIfPresent(payload, "name", wrapper.getAccessibleName());
            }
            Map<String, Object> bounds = SwingGeometry.boundsWithin(header, header.getHeaderRect(column));
            if (bounds != null) {
                payload.put("bounds", bounds);
            }
            Map<String, Object> block = Json.newObject();
            block.put("column", Long.valueOf(column));
            // The model index, not the view index: it survives column reordering,
            // so a locator written against it keeps addressing the same data.
            block.put("modelIndex", Long.valueOf(model.getModelIndex()));
            block.put("resizable", Boolean.valueOf(model.getResizable()));
            payload.put("columnHeader", block);
        } else if (wrapper != null) {
            putIfPresent(payload, "name", wrapper.getAccessibleName());
        }
        payload.put("enabled", Boolean.valueOf(header.isEnabled()));
        payload.put("visible", Boolean.valueOf(header.isVisible()));
        payload.put("showing", Boolean.valueOf(header.isShowing()));
    }

    /** Bounds of a non-cell virtual child, via its accessible component. */
    private static Map<String, Object> virtualBounds(Component owner, AccessibleContext wrapper) {
        if (owner == null || wrapper == null || wrapper.getAccessibleComponent() == null) {
            return null;
        }
        Rectangle local = wrapper.getAccessibleComponent().getBounds();
        return local == null ? null : SwingGeometry.boundsWithin(owner, local);
    }

    /**
     * The cell's value straight from the table model — a bulk-readable source with no renderer in
     * the way (design decision 1).
     */
    private static String modelValueAt(JTable table, int row, int column) {
        try {
            Object value = table.getValueAt(row, column);
            return value == null ? "" : String.valueOf(value);
        } catch (RuntimeException e) {
            // A model mutating underneath the read; an empty name beats failing
            // the whole enumeration pass over one cell.
            AgentLog.debug("cell value unavailable at " + row + "/" + column + ": " + e);
            return "";
        }
    }

    // ------------------------------------------------------------ accessibility

    /**
     * {@code Component.getName()}, but only when it is the developer's own name.
     *
     * <p>This distinction is load-bearing, and getting it wrong is silently destructive.
     * {@code getName()} does not return {@code null} for an unnamed component — AWT
     * <em>manufactures</em> one on first read: {@code Frame} becomes {@code "frame0"},
     * {@code Dialog} {@code "dialog0"}, and so on for every heavyweight class. Reporting that as the
     * element's name would make every Swing window answer to {@code frame0} instead of to its title,
     * and any locator matching a window by name would stop matching.
     *
     * <p>AWT records which of the two happened, in {@code Component.nameExplicitlySet}, so this is an
     * exact answer rather than a guess about name shapes. The agent has already opened
     * {@code java.desktop/java.awt} to itself, so the field is readable; if it somehow is not, the
     * fallback drops the name for {@link Window} subclasses only — those are the ones where
     * auto-generation is certain, while a {@code JComponent}'s name is {@code null} unless set,
     * because {@code JComponent} does not override {@code constructComponentName()}.
     */
    private static String explicitNameOf(Component component) {
        String name = component.getName();
        if (name == null || name.isEmpty()) {
            return null;
        }
        Boolean explicit = nameWasExplicitlySet(component);
        if (explicit != null) {
            return explicit.booleanValue() ? name : null;
        }
        return component instanceof Window ? null : name;
    }

    /** @return whether the name was set by the application, or {@code null} if unknown */
    private static Boolean nameWasExplicitlySet(Component component) {
        try {
            Field field = Component.class.getDeclaredField("nameExplicitlySet");
            field.setAccessible(true);
            Object value = field.get(component);
            return value instanceof Boolean ? (Boolean) value : null;
        } catch (ReflectiveOperationException | RuntimeException e) {
            return null;
        }
    }

    static AccessibleContext accessibleContextOf(Component component) {
        if (!(component instanceof Accessible)) {
            return null;
        }
        try {
            return component.getAccessibleContext();
        } catch (RuntimeException e) {
            AgentLog.debug("accessible context unavailable for " + component.getClass().getName() + ": " + e);
            return null;
        }
    }

    /**
     * The accessible role, in the bridge's vocabulary.
     *
     * <p>A top-level {@code Window} with no accessible role of its own still has to be a window:
     * that is what makes it a top-level node rather than an unnamed panel.
     */
    private static String roleOf(AccessibleContext context, Component component, boolean isWindow) {
        AccessibleRole role = context == null ? null : context.getAccessibleRole();
        if (role != null) {
            return displayString(role);
        }
        if (isWindow) {
            return component instanceof Frame ? "frame" : "window";
        }
        return "unknown";
    }

    private static List<Object> statesOf(AccessibleContext context) {
        List<Object> states = new ArrayList<Object>();
        AccessibleStateSet set;
        try {
            set = context.getAccessibleStateSet();
        } catch (RuntimeException e) {
            return states;
        }
        if (set == null) {
            return states;
        }
        for (AccessibleState state : set.toArray()) {
            if (state != null) {
                states.add(displayString(state));
            }
        }
        return states;
    }

    /** Roles and states as the locale-independent {@code *_en_US} strings the bridge reports. */
    private static String displayString(javax.accessibility.AccessibleBundle bundle) {
        if (bundle == null) {
            return "unknown";
        }
        try {
            String display = bundle.toDisplayString(Locale.ENGLISH);
            return display == null || display.isEmpty() ? "unknown" : display;
        } catch (RuntimeException e) {
            return "unknown";
        }
    }

    /**
     * The element's text content, or {@code null} when it holds none <em>by kind</em>.
     *
     * <p>The null/empty distinction is the whole contract here: {@code null} means "this element has
     * no text capability", {@code ""} means "it has one and it is currently empty". Consumers derive
     * the TextContent capability from the field's presence, so collapsing the two would make an
     * empty text field look like a label.
     */
    private static String textOf(Component component, AccessibleContext context) {
        if (component instanceof JTextComponent) {
            try {
                String text = ((JTextComponent) component).getText();
                // A text component with no content still *is* one.
                return text == null ? "" : text;
            } catch (RuntimeException e) {
                AgentLog.debug("text unavailable: " + e);
                return null;
            }
        }
        if (context == null) {
            return null;
        }
        AccessibleText text = context.getAccessibleText();
        if (!(text instanceof AccessibleExtendedText)) {
            return null;
        }
        try {
            int count = text.getCharCount();
            return count <= 0 ? "" : ((AccessibleExtendedText) text).getTextRange(0, count);
        } catch (RuntimeException e) {
            return null;
        }
    }

    /**
     * Whether the element's text is editable.
     *
     * <p>A capability marker and nothing more: per the {@code text-input-policy} capability the agent
     * exposes <strong>no</strong> programmatic text write. Text is typed with synthesized keyboard
     * input like everywhere else, so what a real user cannot type into, automation cannot fill.
     *
     * @return the editability, or {@code null} when the element carries no text at all
     */
    private static Boolean editableOf(Component component, AccessibleContext context) {
        if (component instanceof JTextComponent) {
            return Boolean.valueOf(((JTextComponent) component).isEditable());
        }
        if (context == null || context.getAccessibleText() == null) {
            return null;
        }
        AccessibleStateSet states = context.getAccessibleStateSet();
        return Boolean.valueOf(states != null && states.contains(AccessibleState.EDITABLE));
    }

    private static String toolTipOf(Component component) {
        if (!(component instanceof JComponent)) {
            return null;
        }
        try {
            return ((JComponent) component).getToolTipText();
        } catch (RuntimeException e) {
            return null;
        }
    }

    private static Map<String, Object> valueOf(AccessibleContext context) {
        AccessibleValue value = context == null ? null : context.getAccessibleValue();
        if (value == null) {
            return null;
        }
        Map<String, Object> block = Json.newObject();
        putNumber(block, "current", value.getCurrentAccessibleValue());
        putNumber(block, "minimum", value.getMinimumAccessibleValue());
        putNumber(block, "maximum", value.getMaximumAccessibleValue());
        return block.isEmpty() ? null : block;
    }

    /**
     * The selected children, by index <em>and by element id</em>.
     *
     * <p>The ids are the point. A consumer reading "which children are selected" wants to match them
     * against nodes it already holds, so an index alone is only half an answer and an id that names
     * nothing is worse than none. They are therefore taken from {@link SwingTree#childAt}, the same
     * function the tree itself hands children out of — where that cannot guarantee the accessible
     * order and the tree order agree, it returns nothing and so does this.
     *
     * <p>Bounded scan: a list with everything selected must not turn one attribute read into tens of
     * thousands of registrations. Truncation is reported rather than silent, because a caller that
     * sees fewer ids than {@code count} needs to know why.
     *
     * <p>A {@code JTable} takes {@link #tableSelectionOf} instead: its accessible indices address
     * cells, which are no longer its direct children.
     */
    private static Map<String, Object> selectionOf(
            AccessibleContext context, Component owner, ElementRegistry registry) {
        if (owner instanceof JTable) {
            return tableSelectionOf((JTable) owner, registry);
        }
        AccessibleSelection selection = context == null ? null : context.getAccessibleSelection();
        if (selection == null) {
            return null;
        }
        Map<String, Object> block = Json.newObject();
        try {
            block.put("count", Long.valueOf(selection.getAccessibleSelectionCount()));
            List<Object> indices = new ArrayList<Object>();
            List<Object> ids = new ArrayList<Object>();
            boolean idsComplete = true;
            int children = context.getAccessibleChildrenCount();
            for (int index = 0; index < children; index++) {
                if (!selection.isAccessibleChildSelected(index)) {
                    continue;
                }
                if (indices.size() >= SELECTION_SCAN_LIMIT) {
                    idsComplete = false;
                    break;
                }
                indices.add(Long.valueOf(index));
                Object child = SwingTree.childAt(owner, index);
                if (child == null) {
                    idsComplete = false;
                } else {
                    ids.add(Long.valueOf(registry.idFor(child)));
                }
            }
            block.put("indices", indices);
            // Only a complete, trustworthy list travels: a partial one would look
            // like "these are the selected children" while omitting some.
            if (idsComplete) {
                block.put("ids", ids);
            }
        } catch (RuntimeException e) {
            return null;
        }
        return block;
    }

    /**
     * A table's selection, taken from the table rather than from its accessible view.
     *
     * <p>{@code AccessibleSelection} on a {@code JTable} answers in terms of accessible child
     * indices, and those address <strong>cells</strong> — which stopped being the table's direct
     * children when rows arrived. Rather than translate one shape into the other, the answer is
     * taken from where it is unambiguous: {@code getSelectedRows} is the table's own account of what
     * is selected, and the ids therefore name rows, which is what row selection means.
     *
     * <p>Cell-level selection does not disappear; it lives on the cell, where {@code isCellSelected}
     * puts it.
     */
    private static Map<String, Object> tableSelectionOf(JTable table, ElementRegistry registry) {
        int[] rows;
        try {
            rows = table.getSelectedRows();
        } catch (RuntimeException e) {
            return null;
        }
        Map<String, Object> block = Json.newObject();
        block.put("count", Long.valueOf(rows == null ? 0 : rows.length));
        List<Object> indices = new ArrayList<Object>();
        List<Object> ids = new ArrayList<Object>();
        boolean idsComplete = true;
        if (rows != null) {
            for (int row : rows) {
                if (indices.size() >= SELECTION_SCAN_LIMIT) {
                    idsComplete = false;
                    break;
                }
                indices.add(Long.valueOf(row));
                ids.add(Long.valueOf(registry.idFor(SwingTree.internRow(table, row))));
            }
        }
        block.put("indices", indices);
        if (idsComplete) {
            block.put("ids", ids);
        }
        return block;
    }

    private static Map<String, Object> tableOf(Component component, AccessibleContext context) {
        if (component instanceof JTable) {
            JTable table = (JTable) component;
            Map<String, Object> block = Json.newObject();
            block.put("rows", Long.valueOf(table.getRowCount()));
            block.put("columns", Long.valueOf(table.getColumnCount()));
            block.put("selectedRows", asLongList(table.getSelectedRows()));
            block.put("selectedColumns", asLongList(table.getSelectedColumns()));
            return block;
        }
        AccessibleTable table = context == null ? null : context.getAccessibleTable();
        if (table == null) {
            return null;
        }
        Map<String, Object> block = Json.newObject();
        block.put("rows", Long.valueOf(table.getAccessibleRowCount()));
        block.put("columns", Long.valueOf(table.getAccessibleColumnCount()));
        return block;
    }

    private static List<Object> asLongList(int[] values) {
        List<Object> list = new ArrayList<Object>();
        if (values != null) {
            for (int value : values) {
                list.add(Long.valueOf(value));
            }
        }
        return list;
    }

    // ------------------------------------------------------------------ helpers

    private static void putIfPresent(Map<String, Object> target, String key, String value) {
        if (value != null && !value.isEmpty()) {
            target.put(key, value);
        }
    }

    private static void putNumber(Map<String, Object> target, String key, Number value) {
        if (value != null) {
            target.put(key, Double.valueOf(value.doubleValue()));
        }
    }

    /**
     * Swing client properties, the place enterprise applications habitually stash their own
     * automation ids.
     *
     * <p>Reachable only reflectively — {@code JComponent} exposes a getter per key but no key list —
     * so this is best effort by construction: on 9+ without
     * {@code --add-opens java.desktop/javax.swing=ALL-UNNAMED} the block is simply absent, which is
     * a missing convenience and not a broken element.
     */
    private static final class SwingClientProperties {

        private SwingClientProperties() {
            // Static helper.
        }

        static Map<String, Object> of(Component component) {
            if (!(component instanceof JComponent)) {
                return null;
            }
            Object table = clientPropertyTable((JComponent) component);
            if (table == null) {
                return null;
            }
            Object[] keys = keysOf(table);
            if (keys == null) {
                return null;
            }
            Map<String, Object> properties = Json.newObject();
            for (Object key : keys) {
                if (key == null) {
                    continue;
                }
                Object value = ((JComponent) component).getClientProperty(key);
                // Only scalars: a client property can hold any object, and
                // stringifying a listener or a UI delegate would be noise at best.
                if (value instanceof String || value instanceof Number || value instanceof Boolean) {
                    properties.put(String.valueOf(key), value instanceof Number
                            ? Double.valueOf(((Number) value).doubleValue())
                            : value);
                }
            }
            return properties;
        }

        private static Object clientPropertyTable(JComponent component) {
            try {
                Field field = JComponent.class.getDeclaredField("clientProperties");
                field.setAccessible(true);
                return field.get(component);
            } catch (ReflectiveOperationException | RuntimeException e) {
                return null;
            }
        }

        private static Object[] keysOf(Object table) {
            try {
                Method getKeys = table.getClass().getDeclaredMethod("getKeys", java.util.Vector.class);
                getKeys.setAccessible(true);
                Object keys = getKeys.invoke(table, (Object) null);
                return keys instanceof Object[] ? (Object[]) keys : null;
            } catch (ReflectiveOperationException | RuntimeException e) {
                return null;
            }
        }
    }

    /** Whether {@code container} has child components worth walking. */
    static boolean hasComponentChildren(Component component) {
        return component instanceof Container && ((Container) component).getComponentCount() > 0;
    }
}
