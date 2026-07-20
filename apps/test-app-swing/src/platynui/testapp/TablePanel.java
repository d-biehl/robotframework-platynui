package platynui.testapp;

import java.awt.FlowLayout;
import javax.swing.BorderFactory;
import javax.swing.JPanel;
import javax.swing.JScrollPane;
import javax.swing.JTable;
import javax.swing.table.DefaultTableModel;

/**
 * Table controls: a fixed-size, read-only {@code JTable} — the acceptance carrier for the JAB
 * table-interface attributes ({@code native:Table.*} / {@code native:TableCell.*}).
 *
 * <p>Fixed accessible names (never change): {@code table-panel}, {@code table-scroll},
 * {@code main-table}. The table has exactly {@value #ROWS}&times;{@value #COLUMNS} data cells with
 * content {@code r&lt;row&gt;c&lt;column&gt;}. Note that cell content does NOT work as a locator
 * anchor through the Java Access Bridge: the JDK-side bridge resolves every JTable cell to the one
 * shared cell-renderer component, so a cell's accessible name reads whatever cell was configured
 * last — tests address cells by their row-major child position and assert content-independent
 * facts via the {@code native:TableCell.*} attributes. Row {@value #SELECTED_ROW} is preselected
 * (row selection), so its cells report {@code isSelected = true} while every other cell reports
 * {@code false}; tests must not change the selection.
 */
final class TablePanel extends JPanel {

    private static final long serialVersionUID = 1L;

    static final int ROWS = 4;
    static final int COLUMNS = 3;
    static final int SELECTED_ROW = 2;

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
        table.setPreferredScrollableViewportSize(table.getPreferredSize());

        JScrollPane scrollPane = new JScrollPane(table);
        scrollPane.getAccessibleContext().setAccessibleName("table-scroll");
        add(scrollPane);
    }
}
