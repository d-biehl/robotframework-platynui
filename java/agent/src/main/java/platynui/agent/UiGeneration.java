package platynui.agent;

import java.util.Map;
import java.util.concurrent.atomic.AtomicLong;

/**
 * A counter that goes up whenever the UI's structure changed (design decision 7).
 *
 * <p>It is an <strong>invalidation hint</strong>, not a source of truth: a client that sees a new
 * generation knows its cached tree may be stale, and nothing more. Per-element validity is answered
 * by {@link ElementRegistry#isLive}, never inferred from this — a counter cannot tell you *which*
 * element went away, and guessing would either over-invalidate every cache or miss the one node
 * that actually died.
 *
 * <p>Published over the reserved notification frame. Coalesced, because structural churn during a
 * window opening would otherwise produce a burst of frames that says exactly as much as one frame:
 * "look again".
 */
final class UiGeneration {

    /** Shortest gap between two published notifications. */
    private static final long PUBLISH_INTERVAL_MS = 200L;

    /** The notification method name. Part of the wire from version 1. */
    static final String NOTIFICATION = "ui/generation";

    private final AtomicLong generation = new AtomicLong();
    private final RpcServer server;

    private volatile long published;
    private volatile boolean running;

    UiGeneration(RpcServer server) {
        this.server = server;
    }

    long current() {
        return generation.get();
    }

    /** Records a structural change. Cheap enough to call from a toolkit event listener. */
    void bump() {
        generation.incrementAndGet();
    }

    /** Starts the coalescing publisher; idempotent. */
    synchronized void startPublishing() {
        if (running) {
            return;
        }
        running = true;
        Thread publisher = new Thread(new Runnable() {
            @Override
            public void run() {
                publishLoop();
            }
        }, "platynui-agent-generation");
        publisher.setDaemon(true);
        publisher.start();
    }

    synchronized void stopPublishing() {
        running = false;
    }

    private void publishLoop() {
        while (running) {
            try {
                Thread.sleep(PUBLISH_INTERVAL_MS);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                return;
            }
            long now = generation.get();
            if (now == published) {
                continue;
            }
            published = now;
            Map<String, Object> params = Json.newObject();
            params.put("generation", Long.valueOf(now));
            server.broadcast(NOTIFICATION, params);
        }
    }
}
