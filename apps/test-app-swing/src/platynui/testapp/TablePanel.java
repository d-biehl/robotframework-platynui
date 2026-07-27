package platynui.testapp;

import java.awt.Dimension;
import java.awt.FlowLayout;
import javax.swing.BorderFactory;
import javax.swing.JPanel;
import javax.swing.JScrollPane;
import javax.swing.JTable;
import javax.swing.table.DefaultTableModel;

/**
 * Table controls: a fixed-size, read-only {@code JTable} — the acceptance carrier for the table
 * attributes ({@code native:Table.*} / {@code native:TableRow.*} / {@code native:TableCell.*}) on
 * both Java backends.
 *
 * <p>Fixed accessible names (never change): {@code table-panel}, {@code table-scroll},
 * {@code main-table}. The table has exactly {@value #ROWS}&times;{@value #COLUMNS} data cells with
 * content {@code r&lt;row&gt;c&lt;column&gt;} and headers {@code col-&lt;column&gt;}, so every cell
 * name is unique across the fixture. Row {@value #SELECTED_ROW} is preselected (row selection), so
 * its cells report {@code isSelected = true} while every other cell reports {@code false}; tests
 * must not change the selection.
 *
 * <p><strong>It scrolls, deliberately.</strong> The viewport shows {@value #VISIBLE_ROWS} of the
 * rows and {@value #VISIBLE_COLUMNS} of the columns, and auto-resize is off, so both scrollbars are
 * present and most of the content is off-view at any moment. A table that fits its viewport is the
 * easy case; a real one does not, and the interesting questions — what bounds a scrolled-out row
 * reports, what a walk of the whole model costs — only exist once it does not.
 *
 * <p>Note that cell content does NOT work as a locator anchor through the Java Access Bridge: the
 * JDK-side bridge resolves every JTable cell to the one shared cell-renderer component, so a cell's
 * accessible name reads whatever cell was configured last — bridge-facing tests address cells by
 * their row-major child position ({@code row * }{@value #COLUMNS}{@code  + column}, 1-based in
 * XPath) and assert content-independent facts via the {@code native:TableCell.*} attributes. Through
 * the in-JVM agent the name is the model value and is stable, and the cells sit beneath row nodes
 * rather than directly beneath the table.
 */
final class TablePanel extends JPanel {

    private static final long serialVersionUID = 1L;

    static final int ROWS = 100;
    static final int COLUMNS = 6;
    static final int SELECTED_ROW = 2;
    static final int VISIBLE_ROWS = 8;
    static final int VISIBLE_COLUMNS = 4;

    TablePanel() {
        super(new FlowLayout(FlowLayout.LEFT, 8, 8));
        getAccessibleContext().setAccessibleName("table-panel");
        setBorder(BorderFactory.createTitledBorder("Table"));

        String[] columns = new String[COLUMNS];
        String[][] data = new String[ROWS][COLUMNS];
        for (int column = 0; column < COLUMNS; column++) {
            columns[column] = "col-" + column;
            for (int row = 0; row < ROWS; row++) {
                data[row][column] = "r" + row + "c" + column;
            }
        }

        JTable table = new JTable(new DefaultTableModel(data, columns) {
            private static final long serialVersionUID = 1L;

            @Override
            public boolean isCellEditable(int row, int column) {
                return false; // keep the fixture state deterministic
            }
        });
        table.getAccessibleContext().setAccessibleName("main-table");
        table.setRowSelectionInterval(SELECTED_ROW, SELECTED_ROW);
        // Columns keep their width instead of being squeezed into the viewport,
        // which is what makes the horizontal scrollbar appear at all.
        table.setAutoResizeMode(JTable.AUTO_RESIZE_OFF);

        int columnWidth = table.getColumnModel().getColumn(0).getPreferredWidth();
        table.setPreferredScrollableViewportSize(
                new Dimension(VISIBLE_COLUMNS * columnWidth, VISIBLE_ROWS * table.getRowHeight()));

        JScrollPane scrollPane = new JScrollPane(table);
        scrollPane.getAccessibleContext().setAccessibleName("table-scroll");
        add(scrollPane);
    }
}
