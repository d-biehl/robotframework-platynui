package platynui.agent;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

import java.util.Map;
import javax.swing.JButton;
import javax.swing.JPanel;
import org.junit.jupiter.api.Test;

/**
 * Strings on the wire as Swing reports them (capability {@code attribute-values}).
 *
 * <p>An empty string Swing reports is a value and travels as {@code ""}, so the provider's
 * {@code native:} attributes can show it; only what Swing does not report at all is left out. The
 * provider, not the wire, decides that an empty {@code Id} or {@code Description} is none.
 *
 * <p>No display is needed: every read here is a property of the component, not of the screen.
 */
class SwingValuesAsReportedTest {

    private static final String[] STRING_KEYS = {
        "componentName", "accessibleName", "accessibleDescription", "toolTipText"
    };

    @Test
    void explicitly_empty_strings_travel_as_empty_strings() {
        JButton button = new JButton("Save");
        button.setName("");
        button.getAccessibleContext().setAccessibleName("");
        button.getAccessibleContext().setAccessibleDescription("");
        button.setToolTipText("");

        Map<String, Object> payload = SwingElement.describe(button, new ElementRegistry(), 0);

        for (String key : STRING_KEYS) {
            assertEquals("", payload.get(key), key + " was set to the empty string: " + payload);
        }
    }

    @Test
    void strings_swing_does_not_report_are_left_out() {
        Map<String, Object> payload = SwingElement.describe(new JPanel(), new ElementRegistry(), 0);

        for (String key : STRING_KEYS) {
            assertFalse(payload.containsKey(key), key + " must be absent, not empty: " + payload);
        }
    }
}
