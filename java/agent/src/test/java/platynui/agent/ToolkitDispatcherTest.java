package platynui.agent;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.concurrent.CountDownLatch;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import org.junit.jupiter.api.Test;

/**
 * The deadline is what keeps a frozen target from becoming a hung test run — the exact failure JAB
 * has against a wedged JVM, and the one this transport exists not to reproduce. These tests wedge a
 * stand-in toolkit thread on purpose.
 */
class ToolkitDispatcherTest {

    /** A single-threaded stand-in for an EDT, which a test can freeze at will. */
    private static final class FakeToolkitThread implements ToolkitDispatcher, AutoCloseable {

        private final LinkedBlockingQueue<Runnable> queue = new LinkedBlockingQueue<>();
        private final Thread thread;
        private final AtomicBoolean running = new AtomicBoolean(true);

        FakeToolkitThread() {
            thread = new Thread(this::pump, "fake-toolkit");
            thread.setDaemon(true);
            thread.start();
        }

        private void pump() {
            while (running.get()) {
                try {
                    Runnable task = queue.poll(50L, TimeUnit.MILLISECONDS);
                    if (task != null) {
                        task.run();
                    }
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    return;
                }
            }
        }

        @Override
        public void submit(Runnable task) {
            queue.add(task);
        }

        @Override
        public boolean isToolkitThread() {
            return Thread.currentThread() == thread;
        }

        @Override
        public String name() {
            return "fake-toolkit";
        }

        @Override
        public void close() {
            running.set(false);
        }
    }

    @Test
    void work_runs_on_the_toolkit_thread_and_returns_its_result() throws Exception {
        try (FakeToolkitThread toolkit = new FakeToolkitThread()) {
            String result = ToolkitDispatcher.Calls.invokeWithDeadline(toolkit, () -> {
                assertTrue(toolkit.isToolkitThread(), "the task must run on the toolkit thread");
                return "answered";
            }, 2_000L);
            assertEquals("answered", result);
        }
    }

    /**
     * The whole point: a wedged toolkit thread must produce an error at the deadline, and the
     * abandoned job must not be interrupted — interrupting a toolkit thread mid-paint is how an
     * observer breaks the application it is observing.
     */
    @Test
    void a_wedged_toolkit_thread_is_abandoned_at_the_deadline_without_being_interrupted() throws Exception {
        try (FakeToolkitThread toolkit = new FakeToolkitThread()) {
            CountDownLatch wedged = new CountDownLatch(1);
            CountDownLatch release = new CountDownLatch(1);
            AtomicBoolean interrupted = new AtomicBoolean(false);
            AtomicBoolean finished = new AtomicBoolean(false);

            toolkit.submit(() -> {
                wedged.countDown();
                try {
                    release.await(10L, TimeUnit.SECONDS);
                } catch (InterruptedException e) {
                    interrupted.set(true);
                    Thread.currentThread().interrupt();
                }
                finished.set(true);
            });
            assertTrue(wedged.await(5L, TimeUnit.SECONDS), "the stand-in toolkit thread must be busy");

            long start = System.nanoTime();
            RpcException error = assertThrows(RpcException.class,
                    () -> ToolkitDispatcher.Calls.invokeWithDeadline(toolkit, () -> "never", 300L));
            long elapsedMs = (System.nanoTime() - start) / 1_000_000L;

            assertEquals(RpcException.DEADLINE_EXCEEDED, error.code());
            assertTrue(error.getMessage().contains("fake-toolkit"), "the diagnostic must name the thread");
            assertTrue(elapsedMs < 5_000L, "the caller must return at its deadline, not when the thread frees up");
            assertFalse(interrupted.get(), "the toolkit thread must not be interrupted");

            // The thread recovers and the agent keeps serving: an abandoned job is
            // discarded, not a poisoned queue.
            release.countDown();
            long deadline = System.currentTimeMillis() + 5_000L;
            while (!finished.get() && System.currentTimeMillis() < deadline) {
                Thread.sleep(20L);
            }
            assertTrue(finished.get());
            assertEquals("later", ToolkitDispatcher.Calls.invokeWithDeadline(toolkit, () -> "later", 2_000L));
        }
    }

    /** Called from the toolkit thread itself, scheduling and waiting would deadlock. */
    @Test
    void a_call_from_the_toolkit_thread_runs_inline_instead_of_deadlocking() throws Exception {
        try (FakeToolkitThread toolkit = new FakeToolkitThread()) {
            String result = ToolkitDispatcher.Calls.invokeWithDeadline(toolkit,
                    () -> ToolkitDispatcher.Calls.invokeWithDeadline(toolkit, () -> "nested", 1_000L), 5_000L);
            assertEquals("nested", result);
        }
    }

    @Test
    void a_failing_task_surfaces_as_an_rpc_error_not_a_deadline() throws Exception {
        try (FakeToolkitThread toolkit = new FakeToolkitThread()) {
            RpcException error = assertThrows(RpcException.class,
                    () -> ToolkitDispatcher.Calls.invokeWithDeadline(toolkit, () -> {
                        throw new IllegalStateException("component is disposed");
                    }, 2_000L));
            assertEquals(RpcException.INTERNAL_ERROR, error.code());
            assertTrue(error.getMessage().contains("component is disposed"));
        }
    }

    @Test
    void the_neutral_dispatcher_runs_inline_for_a_jvm_without_a_toolkit() throws Exception {
        ToolkitDispatcher direct = new ToolkitDispatcher.Direct();
        assertTrue(direct.isToolkitThread());
        assertEquals("inline", ToolkitDispatcher.Calls.invokeWithDeadline(direct, () -> "inline", 1_000L));
    }
}
