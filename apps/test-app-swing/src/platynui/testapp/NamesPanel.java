package platynui.testapp;

import java.awt.FlowLayout;
import java.text.DecimalFormat;
import java.text.DecimalFormatSymbols;
import java.util.Locale;
import javax.swing.BorderFactory;
import javax.swing.JButton;
import javax.swing.JPanel;
import javax.swing.JScrollPane;
import javax.swing.JTable;
import javax.swing.SwingConstants;
import javax.swing.table.DefaultTableCellRenderer;
import javax.swing.table.DefaultTableModel;

/**
 * Name-source coverage: controls whose accessible name differs from every other value a provider
 * could be tempted to report as their name (OpenSpec {@code name-attribute}).
 *
 * <p>Fixed accessible names (never change): {@code names-panel}, {@code names-button},
 * {@code names-table}.
 *
 * <ul>
 *   <li>The button {@code names-button} carries the component name {@code namesButton} (set
 *       through {@code setName}), the accessible description {@code A button with a developer name}
 *       and the visible label {@code Named} — four different strings, so a test can tell which one a
 *       provider reports where.
 *   <li>The read-only table {@code names-table} has one row whose displayed text differs from its
 *       model values: {@code amount} holds the {@code Double} {@code 1234.5} and displays
 *       {@code 1,234.50}, {@code active} holds the {@code Boolean} {@code true} and displays a check
 *       box without text. The amount format is fixed, so the displayed text does not follow the
 *       JVM's default locale.
 * </ul>
 */
final class NamesPanel extends JPanel {

    private static final long serialVersionUID = 1L;

    static final double AMOUNT = 1234.5;

    NamesPanel() {
        super(new FlowLayout(FlowLayout.LEFT, 8, 8));
        getAccessibleContext().setAccessibleName("names-panel");
        setBorder(BorderFactory.createTitledBorder("Names"));

        JButton button = new JButton("Named");
        button.setName("namesButton");
        button.getAccessibleContext().setAccessibleName("names-button");
        button.getAccessibleContext().setAccessibleDescription("A button with a developer name");

        JTable table = new JTable(new DefaultTableModel(
                new Object[][] {{Double.valueOf(AMOUNT), Boolean.TRUE}}, new Object[] {"amount", "active"}) {
            private static final long serialVersionUID = 1L;

            @Override
            public Class<?> getColumnClass(int column) {
                // Boolean.class makes JTable use its own check box renderer.
                return column == 1 ? Boolean.class : Double.class;
            }

            @Override
            public boolean isCellEditable(int row, int column) {
                return false; // keep the fixture state deterministic
            }
        });
        table.getAccessibleContext().setAccessibleName("names-table");
        table.getColumnModel().getColumn(0).setCellRenderer(new AmountRenderer());
        table.setPreferredScrollableViewportSize(table.getPreferredSize());

        add(button);
        add(new JScrollPane(table));
    }

    /** Renders an amount with a fixed {@code #,##0.00} pattern and locale-independent symbols. */
    private static final class AmountRenderer extends DefaultTableCellRenderer {

        private static final long serialVersionUID = 1L;

        private final DecimalFormat format =
                new DecimalFormat("#,##0.00", DecimalFormatSymbols.getInstance(Locale.ROOT));

        AmountRenderer() {
            setHorizontalAlignment(SwingConstants.RIGHT);
        }

        @Override
        protected void setValue(Object value) {
            setText(value instanceof Number ? format.format(value) : "");
        }
    }
}
