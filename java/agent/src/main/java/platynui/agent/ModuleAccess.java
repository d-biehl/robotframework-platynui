package platynui.agent;

import java.lang.instrument.Instrumentation;
import java.lang.reflect.Method;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;

/**
 * Opens the JDK internals the toolkit adapters read, using the agent's own privilege.
 *
 * <h2>Why this exists</h2>
 *
 * <p>The native window handle lives behind {@code java.awt.Component#peer} and
 * {@code sun.awt.windows.WComponentPeer#getHWnd}. On Java 9+ both are closed, and measurement says
 * closed means closed: with neither package opened the handle is unavailable, and with only
 * {@code java.desktop/java.awt} opened it is <em>still</em> unavailable — the field read and the
 * method call each need their own package.
 *
 * <p>The documented remedy is {@code --add-opens} on the command line, which this design cannot use:
 * the whole premise of attaching to a running JVM is that PlatynUI does not own the launch line, and
 * a JVM already running cannot be given the flag retroactively. Without a way out, the in-JVM handle
 * would be dead on every modern JDK and the provider's PID-plus-geometry guess would be the only path
 * — on Windows a workable fallback, on X11 nothing at all.
 *
 * <p>{@link Instrumentation#redefineModule} is the way out, and it is not a trick: JEP 261 gave
 * instrumentation agents this power precisely so they can reach what they need without the
 * application's launch line having to anticipate them. An agent that was allowed into the process is
 * already trusted with more than this.
 *
 * <p>Everything is reflective because the agent compiles to Java 8 bytecode, where {@code Module} does
 * not exist. On Java 8 there is nothing to open and this is a no-op — the handle is reachable there
 * through the deprecated-but-public {@code Component.getPeer()}.
 */
final class ModuleAccess {

    /**
     * Packages of {@code java.desktop} the adapters read.
     *
     * <p>{@code sun.awt.X11} is requested on every platform: it is absent from a Windows JDK's module
     * and is filtered out there, and requesting it here keeps the Linux bring-up from needing a second
     * mechanism.
     */
    private static final String[] DESKTOP_PACKAGES = {
        "java.awt", "sun.awt", "sun.awt.windows", "sun.awt.X11", "javax.swing",
    };

    private ModuleAccess() {
        // Static helper.
    }

    /**
     * Opens {@link #DESKTOP_PACKAGES} to the agent's own module.
     *
     * <p>Best effort and silent about it: a JVM that refuses is a JVM where the provider's fallback
     * takes over, which is a documented degradation and not a failure.
     *
     * @param instrumentation the JVM's instrumentation handle, or {@code null}
     * @return which packages were opened, for the diagnostic — empty on Java 8 and on refusal
     */
    static Set<String> openDesktopInternals(Instrumentation instrumentation) {
        if (instrumentation == null) {
            return Collections.emptySet();
        }
        try {
            Method getModule = Class.class.getMethod("getModule");
            Object desktopModule = getModule.invoke(java.awt.Component.class);
            Object ownModule = getModule.invoke(ModuleAccess.class);
            if (desktopModule == null || ownModule == null) {
                return Collections.emptySet();
            }
            Set<String> present = packagesOf(desktopModule);
            Set<String> requested = new LinkedHashSet<String>();
            Map<String, Set<Object>> extraOpens = new LinkedHashMap<String, Set<Object>>();
            for (String name : DESKTOP_PACKAGES) {
                // `redefineModule` rejects the whole call for a package the module
                // does not have, so a Windows JDK's missing `sun.awt.X11` would take
                // the rest down with it.
                if (present.isEmpty() || present.contains(name)) {
                    extraOpens.put(name, Collections.singleton(ownModule));
                    requested.add(name);
                }
            }
            if (extraOpens.isEmpty()) {
                return Collections.emptySet();
            }
            redefine(instrumentation, desktopModule, extraOpens);
            AgentLog.debug("opened java.desktop packages to the agent: " + requested);
            return requested;
        } catch (ReflectiveOperationException | RuntimeException e) {
            // Java 8 lands here on `Class.getModule()`; anything else is a JVM that
            // said no. Both mean "use the fallback", so neither is an error.
            AgentLog.debug("module opening unavailable: " + e);
            return Collections.emptySet();
        }
    }

    @SuppressWarnings("unchecked")
    private static Set<String> packagesOf(Object module) {
        try {
            Method getPackages = module.getClass().getMethod("getPackages");
            Object packages = getPackages.invoke(module);
            return packages instanceof Set ? (Set<String>) packages : Collections.<String>emptySet();
        } catch (ReflectiveOperationException | RuntimeException e) {
            // Unknown package set: request everything and let the JVM decide.
            return Collections.emptySet();
        }
    }

    private static void redefine(Instrumentation instrumentation, Object module, Map<String, Set<Object>> extraOpens)
            throws ReflectiveOperationException {
        Class<?> moduleClass = Class.forName("java.lang.Module");
        Method redefineModule = Instrumentation.class.getMethod(
                "redefineModule", moduleClass, Set.class, Map.class, Map.class, Set.class, Map.class);
        redefineModule.invoke(
                instrumentation,
                module,
                Collections.emptySet(),
                Collections.emptyMap(),
                extraOpens,
                Collections.emptySet(),
                Collections.emptyMap());
    }
}
