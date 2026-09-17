package platynui.probe;

import java.io.PrintStream;
import java.lang.instrument.Instrumentation;
import java.lang.reflect.Method;
import java.util.Set;

/**
 * Diagnostic agent: reports how many AWT AppContexts the target JVM has and which windows each of
 * them owns, as seen from an agent thread — the question behind an empty ui/windows answer under
 * Java Web Start.
 */
public final class ProbeAgent {

    public static void agentmain(String args, Instrumentation instrumentation) throws Exception {
        PrintStream out = new PrintStream(new java.io.FileOutputStream(args, true), true);
        out.println("=== probe on thread " + Thread.currentThread().getName() + " ===");

        Class<?> appContext = Class.forName("sun.awt.AppContext");
        Method getAppContext = appContext.getMethod("getAppContext");
        Method getAppContexts = appContext.getMethod("getAppContexts");
        Method get = appContext.getMethod("get", Object.class);

        Object mine = getAppContext.invoke(null);
        out.println("this thread's AppContext = " + mine);

        java.awt.Window[] visibleHere = java.awt.Window.getWindows();
        out.println("Window.getWindows() from here: " + visibleHere.length);

        Set<?> all = (Set<?>) getAppContexts.invoke(null);
        out.println("AppContexts in this JVM: " + all.size());
        for (Object context : all) {
            Object list = get.invoke(context, java.awt.Window.class);
            int count = list instanceof java.util.Vector ? ((java.util.Vector<?>) list).size() : -1;
            out.println("  " + context + " -> window list size " + count);
            if (list instanceof java.util.Vector) {
                for (Object weak : (java.util.Vector<?>) list) {
                    Object window = ((java.lang.ref.WeakReference<?>) weak).get();
                    if (window != null) {
                        out.println("      " + window.getClass().getName() + " showing="
                                + ((java.awt.Window) window).isShowing() + " title="
                                + (window instanceof java.awt.Frame ? ((java.awt.Frame) window).getTitle() : "-"));
                    }
                }
            }
        }
        out.close();
    }
}
