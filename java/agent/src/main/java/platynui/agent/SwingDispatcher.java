package platynui.agent;

import java.awt.EventQueue;

/**
 * The Swing/AWT toolkit thread: AWT's event queue.
 *
 * <p>Nothing here is Swing-specific beyond the queue itself — the deadline, the abandon-on-timeout
 * rule and the reentrancy handling all live in {@link ToolkitDispatcher.Calls}, so this adapter
 * cannot get the bounded-call contract subtly wrong on its own.
 */
final class SwingDispatcher implements ToolkitDispatcher {

    @Override
    public void submit(Runnable task) {
        EventQueue.invokeLater(task);
    }

    @Override
    public boolean isToolkitThread() {
        return EventQueue.isDispatchThread();
    }

    @Override
    public String name() {
        return "AWT-EventQueue";
    }
}
