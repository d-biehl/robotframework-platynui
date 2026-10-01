package platynui.agent;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;

import java.text.DecimalFormat;
import java.text.DecimalFormatSymbols;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import javax.swing.JButton;
import javax.swing.JLabel;
import javax.swing.JList;
import javax.swing.JTable;
import javax.swing.table.DefaultTableCellRenderer;
import javax.swing.table.DefaultTableModel;
import org.junit.jupiter.api.Test;

/**
 * Where the payload takes an element's name, description, text and model value from (specs
 * {@code name-attribute}, {@code description-attribute}, {@code textcontent-pattern}).
 *
 * <p>Every field means one thing: {@code componentName} is the name set through {@code setName},
 * {@code accessibleName} what Swing reports as the accessible name, {@code modelValue} a cell's
 * typed model value, and {@code text} only the text Swing provides through an
 * {@code AccessibleText}. Swing's labels, buttons and renderers provide one for HTML text only, so a
 * plain-text element carries no {@code text} at all.
 *
 * <p>No display is needed: the item wrappers configure their renderer on every read, which is model
 * and layout arithmetic, not painting.
 */
class SwingNameSourcesTest {

    private static Map<String, Object> describe(Object element) {
        return SwingElement.describe(element, new ElementRegistry(), 0);
    }

    /**
     * One row whose cells differ in what they display and what they hold: a formatted amount, a
     * boolean drawn as a check box, a short, an integer, a null and a NaN.
     */
    private static JTable fixtureTable() {
        Object[][] data = {{
            Double.valueOf(1234.5), Boolean.TRUE, Short.valueOf((short) 7), Integer.valueOf(42), null,
            Double.valueOf(Double.NaN)
        }};
        String[] columns = {"amount", "active", "count", "total", "empty", "ratio"};
        JTable table = new JTable(new DefaultTableModel(data, columns) {
            private static final long serialVersionUID = 1L;

            @Override
            public Class<?> getColumnClass(int column) {
                return column == 1 ? Boolean.class : Object.class;
            }
        });
        table.getColumnModel().getColumn(0).setCellRenderer(new DefaultTableCellRenderer() {
            private static final long serialVersionUID = 1L;
            private final DecimalFormat format =
                    new DecimalFormat("#,##0.00", DecimalFormatSymbols.getInstance(Locale.ROOT));

            @Override
            protected void setValue(Object value) {
                setText(format.format(value));
            }
        });
        return table;
    }

    private static Map<String, Object> cell(JTable table, int column) {
        Object row = SwingTree.childrenOf(table).get(0);
        return describe(SwingTree.childrenOf(row).get(column));
    }

    @Test
    void a_component_reports_its_component_name_apart_from_its_accessible_name() {
        JButton button = new JButton("Named");
        button.setName("namesButton");
        button.getAccessibleContext().setAccessibleName("names-button");
        button.getAccessibleContext().setAccessibleDescription("A button with a developer name");

        Map<String, Object> payload = describe(button);

        assertEquals("namesButton", payload.get("componentName"), payload.toString());
        assertEquals("names-button", payload.get("accessibleName"));
        assertEquals("A button with a developer name", payload.get("accessibleDescription"));
        assertFalse(payload.containsKey("name"), "the overloaded name field is gone: " + payload);
    }

    @Test
    void a_tool_tip_is_the_description_swing_derives() {
        JButton button = new JButton("Save");
        button.setToolTipText("Save the file");

        assertEquals("Save the file", describe(button).get("accessibleDescription"));
    }

    @Test
    void plain_text_labels_and_buttons_carry_no_text() {
        JLabel label = new JLabel("clicks-0");
        label.getAccessibleContext().setAccessibleName("stage1-status-clicks-0");
        Map<String, Object> labelPayload = describe(label);
        Map<String, Object> buttonPayload = describe(new JButton("Click me"));

        assertFalse(labelPayload.containsKey("text"), "a plain label provides no text: " + labelPayload);
        assertFalse(buttonPayload.containsKey("text"), "a plain button provides no text: " + buttonPayload);
    }

    @Test
    void an_html_label_carries_the_text_its_accessible_text_reports() {
        // The plain AccessibleText of an HTML label reads its document, which starts with a line break.
        assertEquals("\nHi there", describe(new JLabel("<html>Hi <b>there</b></html>")).get("text"));
    }

    @Test
    void a_formatted_cell_is_named_by_its_display_and_keeps_its_model_value() {
        Map<String, Object> amount = cell(fixtureTable(), 0);

        assertEquals("1,234.50", amount.get("accessibleName"), amount.toString());
        assertEquals(Double.valueOf(1234.5), amount.get("modelValue"));
        assertFalse(amount.containsKey("text"), "a plain-text renderer provides no text: " + amount);
        assertFalse(amount.containsKey("name"), "the overloaded name field is gone: " + amount);
    }

    @Test
    void a_boolean_cell_has_neither_name_nor_text_but_keeps_its_model_value() {
        Map<String, Object> active = cell(fixtureTable(), 1);

        assertNull(active.get("accessibleName"), "Swing reports no name for the check box: " + active);
        assertFalse(active.containsKey("text"), "the check box renderer provides no text: " + active);
        assertEquals(Boolean.TRUE, active.get("modelValue"));
    }

    @Test
    void integral_model_values_travel_as_longs() {
        JTable table = fixtureTable();

        assertEquals(Long.valueOf(7L), cell(table, 2).get("modelValue"));
        assertEquals(Long.valueOf(42L), cell(table, 3).get("modelValue"));
    }

    @Test
    void a_null_model_value_is_left_out_and_nan_keeps_its_string_form() {
        JTable table = fixtureTable();
        Map<String, Object> empty = cell(table, 4);

        assertFalse(empty.containsKey("modelValue"), "a null model value is no value: " + empty);
        assertEquals("NaN", cell(table, 5).get("modelValue"));
    }

    @Test
    void a_column_header_is_named_by_its_accessible_name_and_has_no_text() {
        Map<String, Object> header = describe(SwingTree.childrenOf(fixtureTable().getTableHeader()).get(0));

        assertEquals("amount", header.get("accessibleName"), header.toString());
        assertFalse(header.containsKey("text"), "a plain-text header renderer provides no text: " + header);
        assertFalse(header.containsKey("name"), "the overloaded name field is gone: " + header);
    }

    @Test
    void a_list_entry_is_named_by_its_accessible_name_and_only_html_has_text() {
        JList<String> list = new JList<String>(new String[] {"<html>Alpha</html>", "Beta"});
        List<Object> entries = SwingTree.childrenOf(list);
        Map<String, Object> alpha = describe(entries.get(0));
        Map<String, Object> beta = describe(entries.get(1));

        assertEquals("Beta", beta.get("accessibleName"), beta.toString());
        assertFalse(beta.containsKey("text"), "a plain-text entry provides no text: " + beta);
        assertFalse(beta.containsKey("name"), "the overloaded name field is gone: " + beta);
        assertEquals("\nAlpha", alpha.get("text"), alpha.toString());
    }
}
