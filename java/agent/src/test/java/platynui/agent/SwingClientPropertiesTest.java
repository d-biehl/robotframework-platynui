package platynui.agent;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;

import java.util.Map;
import javax.swing.JButton;
import org.junit.jupiter.api.Test;

/**
 * Swing client properties on the wire — the place enterprise applications habitually stash their own
 * automation ids, and the only stable identifier some of them have.
 *
 * <p>There was no test, and the block was empty on every JDK for a signature typo: the lookup asked
 * for {@code ArrayTable.getKeys(Vector)} where the method is {@code getKeys(Object[])}. Nothing
 * failed — an optional block that silently stays empty looks exactly like an application that sets
 * no client properties, which is the common case. So the point of this test is less the mapping than
 * the fact that the mapping produces anything at all.
 *
 * <p>The test JVM is given {@code --add-opens java.desktop/javax.swing=ALL-UNNAMED} in
 * {@code build.gradle.kts}, because here there is no {@code Instrumentation} to open it with — in a
 * target the agent opens it itself through {@link ModuleAccess}, which is why no launch flag is
 * needed there.
 */
class SwingClientPropertiesTest {

    @Test
    void a_components_client_properties_reach_the_payload() {
        JButton button = new JButton("hit me");
        button.putClientProperty("automationId", "the-answer-42");
        button.putClientProperty("retries", Integer.valueOf(3));
        button.putClientProperty("enabledByPolicy", Boolean.TRUE);

        Map<String, Object> payload = SwingElement.describe(button, new ElementRegistry(), 0);

        @SuppressWarnings("unchecked")
        Map<String, Object> properties = (Map<String, Object>) payload.get("clientProperties");
        assertNotNull(properties, "a component with client properties must carry them: " + payload);
        assertEquals("the-answer-42", properties.get("automationId"));
        // Numbers travel as doubles, like every other number on this wire.
        assertEquals(Double.valueOf(3), properties.get("retries"));
        assertEquals(Boolean.TRUE, properties.get("enabledByPolicy"));
    }

    @Test
    void non_scalar_client_properties_are_left_out() {
        JButton button = new JButton("hit me");
        button.putClientProperty("keep", "yes");
        // A client property can hold anything; stringifying a UI delegate or a listener would be
        // noise at best and a leak of internals at worst.
        button.putClientProperty("drop", new Object());

        Map<String, Object> payload = SwingElement.describe(button, new ElementRegistry(), 0);

        @SuppressWarnings("unchecked")
        Map<String, Object> properties = (Map<String, Object>) payload.get("clientProperties");
        assertNotNull(properties);
        assertEquals("yes", properties.get("keep"));
        assertNull(properties.get("drop"), "only scalars belong on the wire: " + properties);
    }

    @Test
    void a_component_without_client_properties_carries_no_block() {
        Map<String, Object> payload = SwingElement.describe(new JButton("plain"), new ElementRegistry(), 0);
        assertNull(payload.get("clientProperties"), "an empty block would be noise on every element");
    }
}
