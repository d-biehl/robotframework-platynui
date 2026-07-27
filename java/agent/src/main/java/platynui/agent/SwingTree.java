package platynui.agent;

import java.awt.Component;
import java.awt.Container;
import java.awt.Point;
import java.awt.Rectangle;
import java.awt.Window;
import java.lang.ref.WeakReference;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.WeakHashMap;
import javax.accessibility.Accessible;
import javax.accessibility.AccessibleContext;
import javax.swing.JList;
import javax.swing.JTable;
import javax.swing.SwingUtilities;

/**
 * The Swing/AWT tree: the instance tree as the spine, with accessibility-only structure grafted
 * where components end (design decision 1).
 *
 * <p>Everything here runs on the AWT event thread — the runtime enforces that and the deadline
 * around it, so no method needs to remember either.
 *
 * <h2>Why virtual children need their own identity</h2>
 *
 * <p>A {@code JTable}'s cells, a {@code JList}'s entries and a {@code JTree}'s rows are not
 * components; the JDK materialises an {@code AccessibleContext} wrapper for them on demand — and
 * hands out a <strong>fresh object on every call</strong>. Registering that wrapper would give the
 * same cell a new id per lookup, which is the opposite of the identity-stable RuntimeIds this change
 * exists to provide. So the registry gets a {@link VirtualChild} instead: interned per
 * {@code (owner, position)}, so the same cell is the same object, held weakly through its owner so it
 * dies with the table rather than pinning it.
 */
final class SwingTree {

    /**
     * Interned virtual children, per owning component.
     *
     * <p>Weak in the key so a discarded table takes its cells with it. The values hold only a weak
     * reference back to the owner, so nothing here keeps a component alive.
     */
    private static final Map<Component, Map<Long, VirtualChild>> VIRTUAL_CHILDREN =
            new WeakHashMap<Component, Map<Long, VirtualChild>>();

    private SwingTree() {
        // Static helper.
    }

    // -------------------------------------------------------------- top level

    /**
     * The JVM's showing top-level windows.
     *
     * <p>{@code Window.getWindows()} is the authoritative list — it is the JVM's own, so it needs no
     * platform window enumeration and works identically wherever the agent runs. Frames and dialogs
     * are both top-level nodes, matching how every other provider presents them.
     */
    static List<Window> windows() {
        List<Window> windows = new ArrayList<Window>();
        Window active = null;
        for (Window window : Window.getWindows()) {
            if (window == null || !window.isShowing()) {
                continue;
            }
            if (active == null && window.isActive()) {
                active = window;
                continue;
            }
            windows.add(window);
        }
        // Active window first: it is the one a hit-test should consider before any
        // window it overlaps, and `Window.getWindows()` defines no z-order at all.
        if (active != null) {
            windows.add(0, active);
        }
        return windows;
    }

    // --------------------------------------------------------------- children

    /** How many children {@link #childrenOf} would report, without building them. */
    static int childCountOf(Object element) {
        if (element instanceof VirtualChild) {
            VirtualChild child = (VirtualChild) element;
            Component owner = child.owner();
            return child.isRow() && owner instanceof JTable ? ((JTable) owner).getColumnCount() : 0;
        }
        if (element instanceof JTable) {
            return ((JTable) element).getRowCount();
        }
        if (!(element instanceof Component)) {
            return 0;
        }
        Component component = (Component) element;
        int spine = visibleComponentChildren(component).size();
        if (spine > 0) {
            return spine;
        }
        return accessibleChildCount(component);
    }

    /**
     * The children of {@code element}: the spine where there is one, accessibility-only structure
     * where the spine ends.
     *
     * <p>The two are not mixed. A component with child components is described by them — grafting its
     * accessible children on top would double every panel, since for ordinary containers the two
     * lists are the same thing seen twice. A component with none is where the interesting case lives:
     * whatever structure it has exists only in the accessible view.
     *
     * <p>{@code JTable} is the exception that proves the rule: it has no child components (except a
     * cell editor while one is open, which must not hide the cells), so its content is grafted
     * unconditionally and read from the model — as <strong>rows</strong>, each holding its own cells.
     * The flat, row-major cell list is what {@code AccessibleContext.getAccessibleChild(i)} offers
     * and all the Access Bridge ever had; the model underneath has rows, and so do the tables the
     * other providers surface.
     */
    static List<Object> childrenOf(Object element) {
        if (element instanceof VirtualChild) {
            VirtualChild child = (VirtualChild) element;
            Component owner = child.owner();
            if (child.isRow() && owner instanceof JTable) {
                return rowCells((JTable) owner, child.row());
            }
            // Every other virtual child is a leaf. A cell's renderer subtree is the
            // shared renderer's, which belongs to no particular cell.
            return Collections.emptyList();
        }
        if (!(element instanceof Component)) {
            return Collections.emptyList();
        }
        Component component = (Component) element;
        if (component instanceof JTable) {
            return tableRows((JTable) component);
        }
        List<Component> spine = visibleComponentChildren(component);
        if (!spine.isEmpty()) {
            return new ArrayList<Object>(spine);
        }
        return accessibleChildren(component);
    }

    private static List<Component> visibleComponentChildren(Component component) {
        if (!(component instanceof Container)) {
            return Collections.emptyList();
        }
        Component[] children;
        try {
            children = ((Container) component).getComponents();
        } catch (RuntimeException e) {
            return Collections.emptyList();
        }
        List<Component> visible = new ArrayList<Component>(children.length);
        for (Component child : children) {
            // Invisible children are the cards of a tab or a collapsed panel: real
            // objects, but not part of the UI a user or a test is looking at. The
            // walk starts at a showing window, so per-child visibility is enough —
            // no ancestor chain has to be re-checked.
            if (child != null && child.isVisible()) {
                visible.add(child);
            }
        }
        return visible;
    }

    private static List<Object> tableRows(JTable table) {
        int rows = table.getRowCount();
        List<Object> children = new ArrayList<Object>(rows);
        for (int row = 0; row < rows; row++) {
            children.add(internRow(table, row));
        }
        return children;
    }

    private static List<Object> rowCells(JTable table, int row) {
        if (row < 0 || row >= table.getRowCount()) {
            return Collections.emptyList();
        }
        int columns = table.getColumnCount();
        List<Object> cells = new ArrayList<Object>(columns);
        for (int column = 0; column < columns; column++) {
            cells.add(internCell(table, row, column));
        }
        return cells;
    }

    /**
     * The rectangle a row occupies, in the table's own coordinate space.
     *
     * <p>Swing has no row-rectangle API, and the union of the row's cell rectangles is exactly what
     * one would be: {@code getCellRect} composes to it, and it is what a user would point at.
     *
     * @return the union, or {@code null} for a table with no columns or an out-of-range row
     */
    static Rectangle rowRect(JTable table, int row) {
        int columns = table.getColumnCount();
        if (columns <= 0 || row < 0 || row >= table.getRowCount()) {
            return null;
        }
        Rectangle rect = table.getCellRect(row, 0, false);
        for (int column = 1; column < columns; column++) {
            rect = rect.union(table.getCellRect(row, column, false));
        }
        return rect;
    }

    /**
     * The part of a table-space rectangle that is actually on screen, or {@code null} when none of
     * it is.
     *
     * <p>A table larger than its viewport is the normal case, and {@code getCellRect} answers from
     * the model regardless: row 90 of a scrolled table reports a rectangle two thousand pixels below
     * the window. Publishing that would aim the pointer at whatever happens to be there — another
     * window, another application — and would let the Element capability resolve on something with
     * no place on screen. That is the same reasoning {@code SwingGeometry.hasArea} applies to
     * unlaid-out components, one level further: absent is the honest answer.
     *
     * <p>Partially scrolled content is <em>clipped</em> rather than dropped, so the rectangle's
     * centre — which is where pointer input aims — stays inside the part the user can actually see.
     */
    static Rectangle visiblePart(JTable table, Rectangle local) {
        if (local == null) {
            return null;
        }
        Rectangle visible = table.getVisibleRect();
        if (visible == null || visible.isEmpty()) {
            // No viewport, or nothing laid out yet; whether the table is on
            // screen at all is `SwingGeometry.boundsWithin`'s question.
            return local;
        }
        Rectangle clipped = local.intersection(visible);
        return clipped.isEmpty() ? null : clipped;
    }

    private static int accessibleChildCount(Component component) {
        AccessibleContext context = SwingElement.accessibleContextOf(component);
        if (context == null) {
            return 0;
        }
        try {
            return Math.max(0, context.getAccessibleChildrenCount());
        } catch (RuntimeException e) {
            return 0;
        }
    }

    private static List<Object> accessibleChildren(Component component) {
        int count = accessibleChildCount(component);
        if (count == 0) {
            return Collections.emptyList();
        }
        List<Object> children = new ArrayList<Object>(count);
        for (int index = 0; index < count; index++) {
            children.add(internIndexed(component, index));
        }
        return children;
    }

    // --------------------------------------------------------------- identity

    /**
     * Key space of the interning map. Cell keys pack {@code row} into the high word, so the two
     * marker bits stay clear for any row count a table could plausibly hold.
     */
    private static final long INDEXED_KEY = 0x8000_0000_0000_0000L;

    private static final long ROW_KEY = 0x4000_0000_0000_0000L;

    /** The interned cell of {@code table} at {@code (row, column)}. */
    static VirtualChild internCell(JTable table, int row, int column) {
        // The key mixes row and column rather than using the accessible child
        // index, because that index depends on the current column count and would
        // alias different cells across a column change.
        long key = (((long) row) << 32) | (column & 0xFFFF_FFFFL);
        return intern(table, key, Flavour.CELL, row, column);
    }

    /** The interned row {@code row} of {@code table}. */
    static VirtualChild internRow(JTable table, int row) {
        return intern(table, ROW_KEY | (row & 0xFFFF_FFFFL), Flavour.ROW, row, -1);
    }

    /**
     * The child {@link #childrenOf} would produce at accessible index {@code index}, or {@code null}
     * when the two orderings are not guaranteed to agree.
     *
     * <p>Exists so anything that answers in terms of accessible child <em>indices</em> — a selection,
     * above all — can name the very objects the tree hands out, instead of inventing identifiers for
     * them. Returning {@code null} rather than guessing is the point: for a component whose children
     * are real components, the accessible order and the component order are two different orders, and
     * a wrong id is worse than a missing one.
     *
     * <p>A {@code JTable} is now one of those components. Its accessible children are still cells in
     * row-major order, while the children this tree hands out are rows, so an accessible index does
     * not address a direct child any more. Rather than translate — which would answer a
     * cell-shaped question with a row-shaped id — it declines, and the table's selection is
     * re-derived from the model instead (see {@code SwingElement}).
     */
    static Object childAt(Component owner, int index) {
        if (owner == null || index < 0) {
            return null;
        }
        if (owner instanceof JTable) {
            return null;
        }
        if (!visibleComponentChildren(owner).isEmpty()) {
            // Spine children: their order is the container's, and nothing
            // guarantees the accessible view enumerates them the same way.
            return null;
        }
        return index < accessibleChildCount(owner) ? internIndexed(owner, index) : null;
    }

    /** The interned accessibility-only child of {@code owner} at {@code index}. */
    static VirtualChild internIndexed(Component owner, int index) {
        return intern(owner, INDEXED_KEY | index, Flavour.INDEXED, -1, index);
    }

    private static VirtualChild intern(Component owner, long key, Flavour flavour, int row, int column) {
        Map<Long, VirtualChild> perOwner = VIRTUAL_CHILDREN.get(owner);
        if (perOwner == null) {
            perOwner = new HashMap<Long, VirtualChild>();
            VIRTUAL_CHILDREN.put(owner, perOwner);
        }
        Long boxed = Long.valueOf(key);
        VirtualChild child = perOwner.get(boxed);
        if (child == null) {
            child = new VirtualChild(owner, flavour, row, column);
            perOwner.put(boxed, child);
        }
        return child;
    }

    /** What a {@link VirtualChild} stands for. */
    enum Flavour {
        /** A table cell, addressed by {@code (row, column)} and read from the model. */
        CELL,
        /** A table row, addressed by its row index; its children are that row's cells. */
        ROW,
        /** An accessibility-only child, addressed by its accessible index. */
        INDEXED
    }

    /**
     * A child with no component of its own, given a stable identity by being interned.
     *
     * <p>Three flavours in one type: a table cell and a table row, both addressed by model
     * coordinates and read from the model, and an accessibility-only child addressed by its index.
     * They differ in where the truth about them lives, not in how they are identified.
     */
    static final class VirtualChild {

        private final WeakReference<Component> owner;
        private final Flavour flavour;
        private final int row;
        private final int column;

        VirtualChild(Component owner, Flavour flavour, int row, int column) {
            this.owner = new WeakReference<Component>(owner);
            this.flavour = flavour;
            this.row = row;
            this.column = column;
        }

        Component owner() {
            return owner.get();
        }

        boolean isCell() {
            return flavour == Flavour.CELL;
        }

        boolean isRow() {
            return flavour == Flavour.ROW;
        }

        int row() {
            return row;
        }

        int column() {
            return column;
        }

        /**
         * Accessible child index — derived for a cell, so a column change cannot alias it.
         *
         * <p>A row has none: the accessible view of a {@code JTable} knows only cells, which is the
         * whole reason the row level has to come from the model.
         */
        int index() {
            Component target = owner();
            if (flavour == Flavour.INDEXED) {
                return column;
            }
            if (flavour == Flavour.CELL && target instanceof JTable) {
                return row * ((JTable) target).getColumnCount() + column;
            }
            return -1;
        }

        /** The JDK's wrapper for this child; a fresh object each time, which is why it is not the id. */
        AccessibleContext accessibleContext() {
            Component target = owner();
            AccessibleContext parent = SwingElement.accessibleContextOf(target);
            int position = index();
            if (parent == null || position < 0) {
                return null;
            }
            try {
                Accessible child = parent.getAccessibleChild(position);
                return child == null ? null : child.getAccessibleContext();
            } catch (RuntimeException e) {
                AgentLog.debug("accessible child " + position + " unavailable: " + e);
                return null;
            }
        }

        /** Whether the position this child stands for still exists. */
        boolean stillInRange() {
            Component target = owner();
            if (target == null) {
                return false;
            }
            if (flavour == Flavour.INDEXED) {
                return column < accessibleChildCount(target);
            }
            if (!(target instanceof JTable)) {
                return false;
            }
            JTable table = (JTable) target;
            if (flavour == Flavour.ROW) {
                return row < table.getRowCount();
            }
            return row < table.getRowCount() && column < table.getColumnCount();
        }
    }

    // -------------------------------------------------------------- liveness

    /**
     * The toolkit's answer to "is this element still live?" (design decision 2).
     *
     * <p>Reachability alone is not enough, and that is the whole reason the registry takes a
     * toolkit-supplied check: a component removed from its container is still strongly held by
     * whoever removed it, so it never gets collected and would report valid forever. What matters is
     * whether it is still attached to a window that exists — {@code isDisplayable()}, which is false
     * the moment a window is disposed.
     *
     * <p>Deliberately not {@code isShowing()}: a component in an unselected tab is hidden, not dead,
     * and invalidating it would make every consumer re-resolve a node that is about to come back.
     */
    static final ElementRegistry.LivenessCheck LIVENESS = new ElementRegistry.LivenessCheck() {
        @Override
        public boolean isLive(Object element) {
            if (element instanceof VirtualChild) {
                VirtualChild child = (VirtualChild) element;
                Component owner = child.owner();
                return owner != null && isAttached(owner) && child.stillInRange();
            }
            if (element instanceof Component) {
                return isAttached((Component) element);
            }
            return false;
        }
    };

    private static boolean isAttached(Component component) {
        if (component instanceof Window) {
            return component.isDisplayable();
        }
        if (!component.isDisplayable()) {
            return false;
        }
        Window window = SwingUtilities.getWindowAncestor(component);
        return window != null && window.isDisplayable();
    }

    // -------------------------------------------------------------- hit-test

    /**
     * The element chain at a physical desktop point, outermost first.
     *
     * <p>In-process this is nearly free — {@code SwingUtilities.getDeepestComponentAt} is a tree walk
     * over rectangles the toolkit already knows — and it reaches what no out-of-process bridge can:
     * the cell under the cursor, not the table that contains it.
     *
     * <p>The agent's whole contribution to the picker is this answer plus the element's bounds.
     * <strong>Drawing the highlight is the platform's job</strong> ({@code HighlightProvider}), and it
     * has to stay there: the reason the Access Bridge cannot highlight a table cell is not that it
     * cannot draw, it is that it has no bounds for the cell to draw around. Supplying bounds fixes
     * that everywhere at once, whereas an agent painting into the target would be changing the
     * application it observes — and would leave it visibly changed if the run died mid-pick.
     *
     * @return the chain from the containing window down to the deepest element, or an empty list when
     *     no window of this JVM covers the point
     */
    static List<Object> chainAt(double deviceX, double deviceY) {
        for (Window window : windows()) {
            Point local = SwingGeometry.toLocal(window, deviceX, deviceY);
            if (local == null || !containsLocal(window, local)) {
                continue;
            }
            List<Object> chain = new ArrayList<Object>();
            chain.add(window);
            Component deepest = deepestAt(window, local);
            if (deepest != null && deepest != window) {
                appendAncestry(chain, window, deepest);
                Point inDeepest = new Point(local);
                SwingUtilities.convertPointToScreen(inDeepest, window);
                SwingUtilities.convertPointFromScreen(inDeepest, deepest);
                chain.addAll(virtualChainAt(deepest, inDeepest));
            }
            return chain;
        }
        return Collections.emptyList();
    }

    private static boolean containsLocal(Window window, Point local) {
        return local.x >= 0 && local.y >= 0 && local.x < window.getWidth() && local.y < window.getHeight();
    }

    private static Component deepestAt(Window window, Point local) {
        try {
            return SwingUtilities.getDeepestComponentAt(window, local.x, local.y);
        } catch (RuntimeException e) {
            AgentLog.debug("hit-test failed: " + e);
            return null;
        }
    }

    /** Fills in the ancestors between {@code window} (exclusive) and {@code deepest} (inclusive). */
    private static void appendAncestry(List<Object> chain, Window window, Component deepest) {
        List<Component> reversed = new ArrayList<Component>();
        for (Component current = deepest; current != null && current != window; current = current.getParent()) {
            reversed.add(current);
        }
        for (int index = reversed.size() - 1; index >= 0; index--) {
            chain.add(reversed.get(index));
        }
    }

    /**
     * The virtual children of {@code component} under a point in its own coordinate space,
     * outermost first.
     *
     * <p>This is where the picker stops being able to point only at containers: a table resolves to
     * a row and then to a cell, a list to an entry. Both come from the same interning table as the
     * enumerated nodes, so the picked element <em>is</em> the node the tree hands out and a consumer
     * revealing the result can place it — which is the property that makes an in-JVM hit-test worth
     * having.
     */
    private static List<VirtualChild> virtualChainAt(Component component, Point local) {
        try {
            if (component instanceof JTable) {
                JTable table = (JTable) component;
                int row = table.rowAtPoint(local);
                int column = table.columnAtPoint(local);
                if (row < 0 || column < 0) {
                    return Collections.emptyList();
                }
                List<VirtualChild> chain = new ArrayList<VirtualChild>(2);
                chain.add(internRow(table, row));
                chain.add(internCell(table, row, column));
                return chain;
            }
            if (component instanceof JList) {
                int index = ((JList<?>) component).locationToIndex(local);
                if (index < 0 || index >= accessibleChildCount(component)) {
                    return Collections.emptyList();
                }
                // `locationToIndex` answers for points past the last entry too, so
                // the cell's own bounds decide.
                Rectangle cell = ((JList<?>) component).getCellBounds(index, index);
                return cell != null && cell.contains(local)
                        ? Collections.<VirtualChild>singletonList(internIndexed(component, index))
                        : Collections.<VirtualChild>emptyList();
            }
        } catch (RuntimeException e) {
            AgentLog.debug("virtual hit-test failed: " + e);
        }
        return Collections.emptyList();
    }

    // --------------------------------------------------------------- actions

    /**
     * Requests focus for an element (design decision 1: {@code Focusable} via {@code requestFocus}).
     *
     * <p>Focus transfer is asynchronous even when requested on the event thread, so this reports that
     * the request went out — not that focus arrived. A caller that needs the outcome re-reads the
     * element, where {@code focused} is the toolkit's own answer.
     *
     * @return whether there was something focusable to ask
     */
    static boolean requestFocus(Object element) {
        Component component = componentOf(element);
        if (component == null || !component.isFocusable() || !component.isShowing()) {
            return false;
        }
        component.requestFocus();
        return true;
    }

    /** The component an element is, or the owner of a virtual child. */
    static Component componentOf(Object element) {
        if (element instanceof VirtualChild) {
            return ((VirtualChild) element).owner();
        }
        return element instanceof Component ? (Component) element : null;
    }

    /** The window an element belongs to, for the window-pattern delegation. */
    static Window windowOf(Object element) {
        Component component = componentOf(element);
        if (component == null) {
            return null;
        }
        return component instanceof Window ? (Window) component : SwingUtilities.getWindowAncestor(component);
    }

    /** The on-screen rectangle of an element, cell and row bounds included. */
    static Map<String, Object> boundsOf(Object element) {
        if (element instanceof VirtualChild) {
            VirtualChild child = (VirtualChild) element;
            Component owner = child.owner();
            if (child.isCell() && owner instanceof JTable) {
                JTable table = (JTable) owner;
                Rectangle cell = table.getCellRect(child.row(), child.column(), false);
                return SwingGeometry.boundsWithin(table, visiblePart(table, cell));
            }
            if (child.isRow() && owner instanceof JTable) {
                JTable table = (JTable) owner;
                return SwingGeometry.boundsWithin(table, visiblePart(table, rowRect(table, child.row())));
            }
            AccessibleContext context = child.accessibleContext();
            if (owner == null || context == null || context.getAccessibleComponent() == null) {
                return null;
            }
            return SwingGeometry.boundsWithin(owner, context.getAccessibleComponent().getBounds());
        }
        return SwingGeometry.boundsOf(componentOf(element));
    }
}
