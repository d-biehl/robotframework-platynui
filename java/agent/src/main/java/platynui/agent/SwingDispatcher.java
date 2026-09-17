package platynui.agent;

import java.awt.EventQueue;
import java.awt.Toolkit;
import java.awt.event.InvocationEvent;

/**
 * The Swing/AWT toolkit thread: AWT's event queue — of one specific toolkit world.
 *
 * <p>"The toolkit thread" is not one per JVM. AWT partitions a JVM into {@code AppContext}s with an
 * event queue each, and {@code EventQueue.invokeLater} resolves the <em>calling thread's</em> — which
 * for an agent thread is either the wrong one or none at all. A task posted there never reaches the
 * components it was meant to read, and waits out its deadline instead. So a dispatcher is bound to
 * the world it serves, and the adapter picks one per element rather than per thread.
 *
 * <p>Nothing here is Swing-specific beyond the queue itself — the deadline, the abandon-on-timeout
 * rule and the reentrancy handling all live in {@link ToolkitDispatcher.Calls}, so this adapter
 * cannot get the bounded-call contract subtly wrong on its own.
 */
final class SwingDispatcher implements ToolkitDispatcher {

    /** The world this dispatcher posts into; {@code null} means "the caller's", as before. */
    private final Object appContext;

    /** The JVM-wide fallback, for a JVM whose worlds cannot be enumerated at all. */
    SwingDispatcher() {
        this(null);
    }

    SwingDispatcher(Object appContext) {
        this.appContext = appContext;
    }

    @Override
    public void submit(Runnable task) {
        if (appContext != null) {
            EventQueue queue = AppContexts.eventQueueOf(appContext);
            if (queue != null) {
                // Posting an InvocationEvent is how the JDK's own cross-context tooling reaches
                // another world's queue; `invokeLater` would resolve this thread's instead, which
                // is exactly the mistake being fixed.
                queue.postEvent(new InvocationEvent(Toolkit.getDefaultToolkit(), task));
                return;
            }
        }
        EventQueue.invokeLater(task);
    }

    @Override
    public boolean isToolkitThread() {
        if (!EventQueue.isDispatchThread()) {
            return false;
        }
        // On *an* event dispatch thread — but whose? Running another world's task inline here would
        // read that world's components from the wrong thread: the same bug, one level down.
        return appContext == null || appContext == AppContexts.current();
    }

    @Override
    public String name() {
        return appContext == null ? "AWT-EventQueue" : "AWT-EventQueue[" + appContext + "]";
    }
}
