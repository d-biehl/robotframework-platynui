package platynui.agent;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.awt.Dimension;
import java.awt.Rectangle;
import java.util.List;
import java.util.Map;
import javax.swing.JScrollPane;
import javax.swing.JTable;
import javax.swing.table.DefaultTableModel;
import org.junit.jupiter.api.Test;

/**
 * The row level of a {@code JTable}, which exists only because the agent reads the model.
 *
 * <p>Swing's accessible projection of a table is a flat, row-major list of cells — that is what the
 * Access Bridge saw and all it could ever see. These tests pin the shape the model supports
 * instead, plus the two consequences that are easy to get wrong: a row and a cell must not collide
 * in the interning table, and {@code childAt} must stop translating accessible indices for a table
 * now that they no longer address a direct child.
 *
 * <p>No display is needed: everything here is model and layout arithmetic. Geometry conversion into
 * desktop pixels needs a showing component and is covered by the live fixture instead.
 */
class SwingTableRowsTest {

    private static final int ROWS = 4;
    private static final int COLUMNS = 3;
    private static final int SELECTED_ROW = 2;

    private static JTable fixtureTable() {
        String[] columns = new String[COLUMNS];
        String[][] data = new String[ROWS][COLUMNS];
        for (int column = 0; column < COLUMNS; column++) {
            columns[column] = "col-" + column;
            for (int row = 0; row < ROWS; row++) {
                data[row][column] = "r" + row + "c" + column;
            }
        }
        JTable table = new JTable(new DefaultTableModel(data, columns));
        table.setRowSelectionInterval(SELECTED_ROW, SELECTED_ROW);
        return table;
    }

    @Test
    void a_tables_children_are_its_rows_and_a_rows_children_are_its_cells() {
        JTable table = fixtureTable();

        List<Object> rows = SwingTree.childrenOf(table);
        assertEquals(ROWS, rows.size(), "a 4x3 table has four rows, not twelve direct cells");
        assertEquals(ROWS, SwingTree.childCountOf(table), "the count must agree without building them");

        for (int row = 0; row < ROWS; row++) {
            SwingTree.VirtualChild rowChild = (SwingTree.VirtualChild) rows.get(row);
            assertTrue(rowChild.isRow());
            assertFalse(rowChild.isCell());
            assertEquals(row, rowChild.row());

            List<Object> cells = SwingTree.childrenOf(rowChild);
            assertEquals(COLUMNS, cells.size(), "row " + row + " must hold one cell per column");
            assertEquals(COLUMNS, SwingTree.childCountOf(rowChild));
            for (int column = 0; column < COLUMNS; column++) {
                SwingTree.VirtualChild cell = (SwingTree.VirtualChild) cells.get(column);
                assertTrue(cell.isCell());
                assertFalse(cell.isRow());
                assertEquals(row, cell.row());
                assertEquals(column, cell.column());
            }
        }
    }

    @Test
    void a_cell_stays_a_leaf() {
        JTable table = fixtureTable();
        assertEquals(0, SwingTree.childrenOf(SwingTree.internCell(table, 1, 2)).size());
        assertEquals(0, SwingTree.childCountOf(SwingTree.internCell(table, 1, 2)));
    }

    /**
     * The identity promise the RuntimeIds rest on — and the one collision worth pinning: a row's
     * key shares the map with the cells' packed coordinates, so row 0 and cell (0, 0) must not be
     * the same interned object.
     */
    @Test
    void rows_are_interned_and_do_not_collide_with_cells() {
        JTable table = fixtureTable();
        assertSame(SwingTree.internRow(table, 1), SwingTree.internRow(table, 1), "the same row is the same object");
        assertNotSame(SwingTree.internRow(table, 0), SwingTree.internRow(table, 1));
        assertNotSame(SwingTree.internRow(table, 0), SwingTree.internCell(table, 0, 0));
        assertSame(SwingTree.internRow(table, 2), SwingTree.childrenOf(table).get(2), "enumeration hands out the same objects");
    }

    /**
     * {@code childAt} exists so an accessible index can name a real node. For a table it no longer
     * can — the accessible children are cells, the tree's children are rows — and a wrong id is
     * worse than a missing one.
     */
    @Test
    void a_table_declines_to_translate_accessible_indices() {
        JTable table = fixtureTable();
        for (int index = 0; index < ROWS * COLUMNS; index++) {
            assertNull(SwingTree.childAt(table, index), "an accessible cell index must not resolve to a row");
        }
    }

    @Test
    void a_rows_rectangle_spans_its_cells() {
        JTable table = fixtureTable();
        Rectangle row = SwingTree.rowRect(table, SELECTED_ROW);
        Rectangle first = table.getCellRect(SELECTED_ROW, 0, false);
        Rectangle last = table.getCellRect(SELECTED_ROW, COLUMNS - 1, false);
        assertEquals(first.x, row.x);
        assertEquals(first.y, row.y);
        assertEquals(last.x + last.width, row.x + row.width, "the row must reach the last cell's right edge");
        assertEquals(first.height, row.height);

        assertNull(SwingTree.rowRect(table, ROWS), "an out-of-range row has no rectangle");
    }

    /**
     * A table larger than its viewport is the normal case, and {@code getCellRect} answers from the
     * model regardless — so without clipping, row 90 of a scrolled table would publish a rectangle
     * far below the window and the pointer would aim at whatever is there.
     */
    @Test
    void content_scrolled_out_of_the_viewport_has_no_visible_part() {
        int rows = 100;
        String[][] data = new String[rows][1];
        for (int row = 0; row < rows; row++) {
            data[row][0] = "r" + row + "c0";
        }
        JTable table = new JTable(new DefaultTableModel(data, new String[] {"col-0"}));
        table.setAutoResizeMode(JTable.AUTO_RESIZE_OFF);

        JScrollPane scrollPane = new JScrollPane(table);
        // A handful of rows tall, so almost the whole model is out of view.
        // Laid out by hand: headless means nothing lays itself out.
        scrollPane.setSize(new Dimension(400, 6 * table.getRowHeight()));
        scrollPane.doLayout();
        scrollPane.getViewport().doLayout();

        assertNotNull(SwingTree.visiblePart(table, SwingTree.rowRect(table, 0)), "the first row is in view");
        assertNull(
                SwingTree.visiblePart(table, SwingTree.rowRect(table, rows - 1)),
                "a row below the fold has no place on screen");

        // Partially scrolled content is clipped rather than dropped, so the
        // rectangle's centre — where pointer input aims — stays inside the part
        // the user can see.
        Rectangle viewport = table.getVisibleRect();
        Rectangle straddling =
                new Rectangle(0, viewport.y + viewport.height - table.getRowHeight(), 100, 4 * table.getRowHeight());
        Rectangle clipped = SwingTree.visiblePart(table, straddling);
        assertNotNull(clipped);
        assertTrue(clipped.height < straddling.height, "the off-view part must be cut away: " + clipped);
        assertTrue(
                table.getVisibleRect().contains(clipped.x + clipped.width / 2, clipped.y + clipped.height / 2),
                "the clipped centre must be inside the viewport: " + clipped);
    }

    @Test
    void a_row_stops_being_live_once_the_model_no_longer_has_it() {
        JTable table = fixtureTable();
        SwingTree.VirtualChild last = SwingTree.internRow(table, ROWS - 1);
        assertTrue(last.stillInRange());
        ((DefaultTableModel) table.getModel()).removeRow(ROWS - 1);
        assertFalse(last.stillInRange(), "a row the model dropped must not report itself in range");
    }

    /**
     * The payload the provider maps into {@code item:TableRow}. The role is synthesised because the
     * accessible view has no opinion about rows, and the states are derived for the same reason —
     * they are what makes the normalised {@code Selectable} surface resolve.
     */
    @Test
    void a_row_payload_carries_its_position_selection_and_derived_role() {
        JTable table = fixtureTable();
        ElementRegistry registry = new ElementRegistry();

        Map<String, Object> selected =
                SwingElement.describe(SwingTree.internRow(table, SELECTED_ROW), registry, COLUMNS);
        assertEquals("table row", selected.get("role"));
        assertEquals(Long.valueOf(COLUMNS), selected.get("childCount"));
        assertNull(selected.get("name"), "a row carries no label of its own");

        @SuppressWarnings("unchecked")
        Map<String, Object> block = (Map<String, Object>) selected.get("tableRow");
        assertEquals(Long.valueOf(SELECTED_ROW), block.get("row"));
        assertEquals(Boolean.TRUE, block.get("selected"));
        assertEquals(List.of("selectable", "selected"), selected.get("states"));

        Map<String, Object> unselected = SwingElement.describe(SwingTree.internRow(table, 0), registry, COLUMNS);
        @SuppressWarnings("unchecked")
        Map<String, Object> other = (Map<String, Object>) unselected.get("tableRow");
        assertEquals(Boolean.FALSE, other.get("selected"));
        assertEquals(List.of("selectable"), unselected.get("states"));
    }

    /**
     * The selection the table itself publishes. {@code AccessibleSelection} answers in cell indices,
     * which stopped addressing a direct child, so the answer comes from the model — and the ids it
     * publishes must be the very row objects the enumeration hands out.
     */
    @Test
    void a_tables_selection_names_the_selected_rows() {
        JTable table = fixtureTable();
        ElementRegistry registry = new ElementRegistry();

        Map<String, Object> payload = SwingElement.describe(table, registry, ROWS);
        @SuppressWarnings("unchecked")
        Map<String, Object> selection = (Map<String, Object>) payload.get("selection");
        assertEquals(Long.valueOf(1L), selection.get("count"));
        assertEquals(List.of(Long.valueOf(SELECTED_ROW)), selection.get("indices"));

        @SuppressWarnings("unchecked")
        List<Object> ids = (List<Object>) selection.get("ids");
        assertEquals(1, ids.size());
        assertSame(
                SwingTree.internRow(table, SELECTED_ROW),
                registry.resolve(((Long) ids.get(0)).longValue()),
                "the published id must resolve to the row node itself");
    }
}
