package platynui.agent;

import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.awt.AWTEvent;
import java.awt.Toolkit;
import java.awt.event.AWTEventListener;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import javax.swing.JLabel;
import javax.swing.JPanel;
import org.junit.jupiter.api.Test;

/**
 * What an AWT event listener actually hears, measured rather than assumed.
 *
 * <p>The agent's structural-change listener drives the UI-generation counter, which is how a client
 * learns its view of the tree is stale. {@code Toolkit.addAWTEventListener} reads like a JVM-wide
 * hook; whether it is one in a JVM with several {@code AppContext}s — the Java Web Start shape — is
 * the question, and a hint that never fires there is worse than no hint, because clients keep a
 * stale tree and nothing says so. Measured here rather than assumed: it does hear every world, and
 * the guess that it would not was wrong.
 *
 * <p>Headless on purpose: no window is ever shown. Adding a child to a container dispatches a
 * {@code ContainerEvent} synchronously whenever anything is listening, which is all this needs.
 *
 * <p><strong>The extra world is always disposed again</strong>, and that is not tidiness. A second
 * {@code AppContext} costs {@code AppContext.getAppContext()} its single-context shortcut, after
 * which it answers {@code null} for every thread whose group chain carries no context of its own —
 * a JUnit worker thread's does not. Leaving one behind makes every later Swing test in this JVM die
 * inside {@code JComponent}, which is how this was found.
 */
class AppContextEventsTest {

    /** How long a task posted into the other toolkit world may take to run. */
    private static final long TASK_TIMEOUT_SECONDS = 10L;

    @Test
    void an_awt_event_listener_hears_every_toolkit_world() throws Exception {
        final AtomicInteger heardHere = new AtomicInteger();
        AWTEventListener listener = new AWTEventListener() {
            @Override
            public void eventDispatched(AWTEvent event) {
                heardHere.incrementAndGet();
            }
        };
        Toolkit.getDefaultToolkit().addAWTEventListener(listener, AWTEvent.CONTAINER_EVENT_MASK);
        try {
            // Baseline: in the world it was registered in, it fires.
            JPanel here = new JPanel();
            here.add(new JLabel("here"));
            assertTrue(heardHere.get() > 0, "the listener must hear its own world");
            final int afterOwnWorld = heardHere.get();

            inNewToolkitWorld(new Runnable() {
                @Override
                public void run() {
                    JPanel elsewhere = new JPanel();
                    elsewhere.add(new JLabel("elsewhere"));
                }
            });

            // The measured answer, and the opposite of the obvious guess: the listener list hangs
            // off the single Toolkit instance rather than off an AppContext, so ONE registration
            // covers the whole JVM. That is why SwingAdapter registers once instead of per world —
            // per world would install a listener per application in a foreign process and bump the
            // counter once per listener for the same event.
            assertTrue(heardHere.get() > afterOwnWorld,
                    "one registration must hear another world's container events too; if this starts "
                            + "failing, the hint has gone deaf for Web Start applications and "
                            + "SwingAdapter has to register per world after all");
        } finally {
            Toolkit.getDefaultToolkit().removeAWTEventListener(listener);
        }
    }

    @Test
    void the_accessors_report_the_worlds_and_their_queues() throws Exception {
        assertTrue(AppContexts.available(), "the AppContext accessors must work on the test JVM");
        final Object mine = AppContexts.current();
        final int before = AppContexts.all().size();

        final AtomicReference<Object> theirs = new AtomicReference<Object>();
        final AtomicInteger seenFromThere = new AtomicInteger();
        final AtomicReference<Object> theirQueue = new AtomicReference<Object>();
        inNewToolkitWorld(new Runnable() {
            @Override
            public void run() {
                Object here = AppContexts.current();
                theirs.set(here);
                seenFromThere.set(AppContexts.all().size());
                theirQueue.set(AppContexts.eventQueueOf(here));
            }
        });

        assertNotNull(theirs.get(), "the new world must be visible from inside it");
        assertNotSame(mine, theirs.get(), "and must not be the world the test thread is in");
        assertTrue(seenFromThere.get() > before,
                "the enumeration must show both worlds, saw " + seenFromThere.get() + " after " + before);
        assertNotNull(theirQueue.get(), "a world must expose the queue that work is posted to");
    }

    /**
     * Runs {@code task} in a toolkit world of its own — a new thread group with its own
     * {@code AppContext}, which is what applet and Web Start runtimes create per application — and
     * disposes that world again before returning.
     */
    private static void inNewToolkitWorld(final Runnable task) throws Exception {
        final CountDownLatch done = new CountDownLatch(1);
        final AtomicReference<Throwable> failure = new AtomicReference<Throwable>();
        final AtomicReference<Object> created = new AtomicReference<Object>();
        Thread thread = new Thread(new ThreadGroup("PlatynUI test world"), new Runnable() {
            @Override
            public void run() {
                try {
                    created.set(Class.forName("sun.awt.SunToolkit").getMethod("createNewAppContext").invoke(null));
                    task.run();
                } catch (Throwable e) {
                    failure.set(e);
                } finally {
                    done.countDown();
                }
            }
        }, "platynui-test-world");
        thread.setDaemon(true);
        thread.start();
        try {
            assertTrue(done.await(TASK_TIMEOUT_SECONDS, TimeUnit.SECONDS), "the other world never ran the task");
            if (failure.get() != null) {
                throw new AssertionError("the task failed in the other toolkit world", failure.get());
            }
        } finally {
            dispose(created.get());
        }
    }

    /** Takes the extra world back out of this JVM; see the class comment for why that matters. */
    private static void dispose(Object appContext) throws Exception {
        if (appContext == null) {
            return;
        }
        // From this thread, never from inside that world: `dispose` refuses its own context.
        appContext.getClass().getMethod("dispose").invoke(appContext);
    }
}
