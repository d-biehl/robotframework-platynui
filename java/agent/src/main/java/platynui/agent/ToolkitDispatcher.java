package platynui.agent;

import java.util.concurrent.Callable;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.FutureTask;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

/**
 * How work reaches the toolkit thread, and how it is kept bounded (design decision 7).
 *
 * <p>UI toolkits demand single-threaded access, which conveniently also serialises concurrent
 * clients: the Inspector and a test run can both be connected, and their reads queue behind each
 * other on the toolkit thread rather than behind a lock at the socket.
 *
 * <p>The deadline is the part that is easy to get wrong. A bare {@code invokeAndWait} against a
 * frozen toolkit thread pins the RPC handler forever, and the client sees a hang instead of an
 * error — exactly the failure mode JAB has against a wedged JVM, and the one this transport exists
 * to avoid. So every call waits with a timeout, and a task that misses it is
 * <strong>abandoned</strong>: it may still run later, but nobody is waiting for its result.
 *
 * <p>The toolkit-specific implementation ships with the adapter that needs it (Swing's
 * {@code EventQueue}, JavaFX's {@code Platform.runLater}). What lives here is the neutral contract
 * and the deadline logic, so every adapter is bounded the same way.
 */
interface ToolkitDispatcher {

    /** Schedules {@code task} on the toolkit thread and returns immediately. */
    void submit(Runnable task);

    /** Whether the calling thread already is the toolkit thread. */
    boolean isToolkitThread();

    /** Diagnostic name, e.g. {@code "AWT-EventQueue"}. */
    String name();

    /**
     * Runs work on the toolkit thread under a deadline.
     *
     * <p>Reentrancy matters: when the caller already <em>is</em> the toolkit thread, scheduling and
     * waiting would deadlock against itself, so the task runs inline.
     */
    final class Calls {

        private Calls() {
            // Static helper.
        }

        static <T> T invokeWithDeadline(ToolkitDispatcher dispatcher, Callable<T> task, long timeoutMs)
                throws RpcException {
            if (dispatcher.isToolkitThread()) {
                return callDirectly(task);
            }
            FutureTask<T> future = new FutureTask<T>(task);
            dispatcher.submit(future);
            try {
                return future.get(timeoutMs, TimeUnit.MILLISECONDS);
            } catch (TimeoutException e) {
                // No interrupt: interrupting a toolkit thread mid-paint is how an
                // observer breaks the application it is observing. The task is left to
                // finish into a result nobody reads.
                future.cancel(false);
                throw new RpcException(RpcException.DEADLINE_EXCEEDED,
                        "the " + dispatcher.name() + " thread did not answer within " + timeoutMs + " ms");
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                throw new RpcException(RpcException.INTERNAL_ERROR, "interrupted while waiting for the toolkit thread");
            } catch (ExecutionException e) {
                throw asRpcException(e.getCause());
            }
        }

        private static <T> T callDirectly(Callable<T> task) throws RpcException {
            try {
                return task.call();
            } catch (Exception e) {
                throw asRpcException(e);
            }
        }

        private static RpcException asRpcException(Throwable cause) {
            if (cause instanceof RpcException) {
                return (RpcException) cause;
            }
            return new RpcException(RpcException.INTERNAL_ERROR, String.valueOf(cause));
        }
    }

    /**
     * The neutral default: no toolkit thread, work runs on the calling RPC thread.
     *
     * <p>What a JVM without a UI gets — and what the agent uses until an adapter installs a real
     * dispatcher. It is deliberately not a no-op that pretends to be a toolkit thread.
     */
    final class Direct implements ToolkitDispatcher {

        @Override
        public void submit(Runnable task) {
            task.run();
        }

        @Override
        public boolean isToolkitThread() {
            return true;
        }

        @Override
        public String name() {
            return "direct";
        }
    }
}
