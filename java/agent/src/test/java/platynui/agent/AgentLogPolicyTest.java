package platynui.agent;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.lang.reflect.Method;
import java.security.Permission;
import java.util.PropertyPermission;
import org.junit.jupiter.api.Test;

/**
 * The diagnostic channel has to survive the policy of the process it was injected into.
 *
 * <p>This is not hypothetical: measured against a real OpenWebStart application, the agent's first
 * failure was a denied {@code getenv}, and the report of that failure <em>threw its own failure</em>
 * — {@code AgentLog}'s static initializer reads a system property, the JNLP policy denied it, and
 * the resulting {@code ExceptionInInitializerError} escaped {@code agentmain} into the target. The
 * operator was left with a bare "Agent failed to start!" and no cause anywhere.
 *
 * <p>So the property worth pinning is narrow and absolute: initializing {@code AgentLog} must not
 * throw when the property read is denied. A fresh class loader is what makes it testable — the class
 * is long since initialized in this JVM, and a static initializer runs once per loader.
 */
// The agent targets Java 8, where a SecurityManager is ordinary; the test toolchain is a JDK that
// has deprecated it. Reproducing the target's condition is the point, so the warnings are muted
// here rather than the condition avoided.
@SuppressWarnings({"deprecation", "removal"})
class AgentLogPolicyTest {

    private static final String DEBUG_PROPERTY = "platynui.agent.debug";

    @Test
    void the_diagnostic_channel_initialises_when_its_property_read_is_denied() throws Exception {
        // Built before the manager is installed: creating a class loader is itself permission-gated,
        // and this test is about the property read, not about how much else it could have denied.
        IsolatedLoader loader = new IsolatedLoader();

        SecurityManager denyDebugProperty = new SecurityManager() {
            @Override
            public void checkPermission(Permission permission) {
                if (permission instanceof PropertyPermission && DEBUG_PROPERTY.equals(permission.getName())) {
                    throw new SecurityException("denied by " + AgentLogPolicyTest.class.getSimpleName());
                }
            }
        };
        System.setSecurityManager(denyDebugProperty);
        try {
            Class<?> agentLog = Class.forName("platynui.agent.AgentLog", true, loader);
            assertFalse(agentLog.getClassLoader() == AgentLog.class.getClassLoader(),
                    "the test must initialize its own copy, or it proves nothing");

            Method isDebugEnabled = agentLog.getDeclaredMethod("isDebugEnabled");
            isDebugEnabled.setAccessible(true);
            assertEquals(Boolean.FALSE, isDebugEnabled.invoke(null),
                    "a denied read is 'no debug', never an exception");
        } finally {
            System.setSecurityManager(null);
        }
    }

    /**
     * Defines {@code platynui.agent.*} itself instead of delegating, so their static initializers
     * run again under the security manager this test installs.
     */
    private static final class IsolatedLoader extends ClassLoader {

        IsolatedLoader() {
            super(AgentLogPolicyTest.class.getClassLoader());
        }

        @Override
        protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
            if (!name.startsWith("platynui.agent.")) {
                return super.loadClass(name, resolve);
            }
            Class<?> loaded = findLoadedClass(name);
            if (loaded == null) {
                byte[] bytes = bytecodeOf(name);
                loaded = defineClass(name, bytes, 0, bytes.length);
            }
            if (resolve) {
                resolveClass(loaded);
            }
            return loaded;
        }

        private byte[] bytecodeOf(String name) throws ClassNotFoundException {
            String resource = name.replace('.', '/') + ".class";
            try (InputStream stream = getParent().getResourceAsStream(resource)) {
                if (stream == null) {
                    throw new ClassNotFoundException(name);
                }
                ByteArrayOutputStream buffer = new ByteArrayOutputStream();
                byte[] chunk = new byte[8192];
                for (int read = stream.read(chunk); read >= 0; read = stream.read(chunk)) {
                    buffer.write(chunk, 0, read);
                }
                return buffer.toByteArray();
            } catch (IOException e) {
                throw new ClassNotFoundException(name, e);
            }
        }
    }
}
