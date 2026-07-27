package platynui.agent;

import java.io.IOException;
import java.lang.instrument.Instrumentation;
import java.nio.charset.Charset;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Owns everything the agent runs after injection: toolkit detection, the loopback RPC server, and
 * the handshake file that publishes it.
 *
 * <p>Deliberately separate from {@link Agent}, which is the JVM-facing entry point and stays
 * trivial: a failure there is a failure the target application sees.
 */
final class AgentRuntime {

    private static final Charset UTF_8 = Charset.forName("UTF-8");

    private static AgentRuntime instance;

    /** Default agent-side deadline for one toolkit-thread call. */
    private static final long DEFAULT_CALL_DEADLINE_MS = 4_000L;

    private final Instrumentation instrumentation;
    private final String token;
    /**
     * The dispatch table, shared live with the server.
     *
     * <p>Concurrent and mutable rather than a fixed map, because a toolkit adapter contributes its
     * methods when the toolkit appears — which with {@code -javaagent} is after the server is already
     * listening.
     */
    private final Map<String, RpcMethod> methods;

    private final RpcServer server;
    private final long pid;
    private final ElementRegistry registry = new ElementRegistry();
    private final UiGeneration generation;

    /**
     * How work reaches the toolkit thread. Neutral until a toolkit adapter installs its own; a JVM
     * without a UI keeps it.
     */
    private volatile ToolkitDispatcher dispatcher = new ToolkitDispatcher.Direct();

    /**
     * The toolkit adapter, once one applies to this JVM.
     *
     * <p>Late by necessity: injected with {@code -javaagent} the agent runs before the application has
     * loaded a toolkit class, so at construction time there is nothing to adapt to yet.
     */
    private volatile SwingAdapter swingAdapter;

    private HandshakeFile handshake;
    private int port;

    private AgentRuntime(Instrumentation instrumentation) {
        this.instrumentation = instrumentation;
        this.token = newToken();
        this.pid = AgentPaths.currentPid();
        this.methods = buildMethods();
        this.server = new RpcServer(methods, token);
        this.generation = new UiGeneration(server);
    }

    /** The live runtime, or {@code null} if the agent never started. */
    static synchronized AgentRuntime current() {
        return instance;
    }

    /** The element-identity registry an adapter hands ids out of. */
    ElementRegistry registry() {
        return registry;
    }

    /** The UI-generation counter an adapter bumps on structural toolkit events. */
    UiGeneration generation() {
        return generation;
    }

    /** Installs the toolkit's dispatcher; called by an adapter once it knows the toolkit. */
    void setToolkitDispatcher(ToolkitDispatcher toolkitDispatcher) {
        dispatcher = toolkitDispatcher == null ? new ToolkitDispatcher.Direct() : toolkitDispatcher;
    }

    /**
     * Starts the agent runtime.
     *
     * <p>Failures are reported and swallowed: a target application must not die because PlatynUI
     * could not open a socket. What the operator gets instead is a JVM without a reachable agent,
     * which the provider reports as "no agent here" — the same state as a JVM that was never
     * injected.
     */
    static synchronized void start(String args, Instrumentation instrumentation) {
        if (instance != null) {
            return;
        }
        AgentRuntime runtime = new AgentRuntime(instrumentation);
        try {
            runtime.startInternal();
            instance = runtime;
        } catch (IOException | RuntimeException e) {
            AgentLog.error("agent " + Agent.version() + " failed to start (args=" + args + ")", e);
            runtime.stop();
        }
    }

    private void startInternal() throws IOException {
        Path directory = AgentPaths.createHandshakeDirectory();
        List<String> toolkits = detectToolkits();
        // Before the port is published: a client that reads the handshake file and
        // connects immediately must find the tree methods already there.
        installToolkitAdapter(toolkits);
        port = server.start();
        handshake = new HandshakeFile(directory, pid, port, token, Agent.version(), toolkits);
        handshake.publish();
        Runtime.getRuntime().addShutdownHook(new Thread(new Runnable() {
            @Override
            public void run() {
                stop();
            }
        }, "platynui-agent-shutdown"));
        startToolkitWatcher();
        generation.startPublishing();
        AgentLog.debug("agent " + Agent.version() + " listening on 127.0.0.1:" + port
                + ", handshake file " + handshake.path());
    }

    synchronized void stop() {
        generation.stopPublishing();
        if (handshake != null) {
            handshake.remove();
        }
        server.stop();
    }

    // ------------------------------------------------------------- detection

    private List<String> detectToolkits() {
        Set<String> detected = ToolkitDetector.detect(instrumentation);
        return ToolkitDetector.asSortedList(detected);
    }

    /**
     * Keeps the published toolkit set honest right after injection.
     *
     * <p>Injected with {@code -javaagent} the agent runs <em>before</em> the application has loaded
     * a single toolkit class, so the first detection is legitimately empty. A short watcher with
     * backoff fills that in; once a toolkit has shown up the watcher stops, and later changes are
     * picked up on demand by {@code agent/info}. Polling forever would mean a full loaded-class
     * scan every few seconds for the life of a foreign process, which is not a cost an agent gets
     * to impose.
     */
    private void startToolkitWatcher() {
        if (!handshake.toolkits().isEmpty()) {
            return;
        }
        Thread watcher = new Thread(new Runnable() {
            @Override
            public void run() {
                long delayMs = 100L;
                long waitedMs = 0L;
                final long giveUpAfterMs = 120_000L;
                while (waitedMs < giveUpAfterMs) {
                    try {
                        Thread.sleep(delayMs);
                    } catch (InterruptedException e) {
                        Thread.currentThread().interrupt();
                        return;
                    }
                    waitedMs += delayMs;
                    delayMs = Math.min(delayMs * 2L, 5_000L);
                    if (refreshToolkits()) {
                        return;
                    }
                }
                AgentLog.debug("no UI toolkit showed up within " + giveUpAfterMs + "ms; "
                        + "reporting the set on demand from now on");
            }
        }, "platynui-agent-toolkit-watch");
        watcher.setDaemon(true);
        watcher.start();
    }

    /**
     * Re-detects the toolkits and republishes the handshake file if the set changed.
     *
     * @return {@code true} if a toolkit is now known
     */
    private boolean refreshToolkits() {
        List<String> detected = detectToolkits();
        // The `-javaagent` case: the agent started before the application had
        // loaded a toolkit class, so this is where the adapter actually arrives.
        installToolkitAdapter(detected);
        try {
            if (handshake.updateToolkits(detected)) {
                // A toolkit coming up is a structural UI change by definition, and the first
                // real producer of the counter: a client connected before the application's
                // window existed learns that its (empty) view of the world is stale.
                generation.bump();
            }
        } catch (IOException e) {
            AgentLog.error("could not republish the handshake file", e);
        }
        return !detected.isEmpty();
    }

    // ---------------------------------------------------------------- methods

    private Map<String, RpcMethod> buildMethods() {
        Map<String, RpcMethod> table = new ConcurrentHashMap<String, RpcMethod>();
        table.put("handshake", new HandshakeMethod());
        table.put("ping", new PingMethod());
        table.put("agent/info", new InfoMethod());
        table.put("element/live", new LivenessMethod());
        table.put("ui/generation", new GenerationMethod());
        table.put("agent/process", new ProcessMethod());
        return table;
    }

    /**
     * Installs the toolkit adapter for {@code toolkits}, once.
     *
     * <p>Idempotent and late-safe: called after the first detection and again whenever the watcher
     * sees the set change, because a {@code -javaagent}-injected agent starts before the application
     * has loaded a single toolkit class.
     */
    private synchronized void installToolkitAdapter(List<String> toolkits) {
        if (swingAdapter != null || toolkits.isEmpty()) {
            return;
        }
        // Before any adapter reads a JDK internal: on Java 9+ the packages the
        // native window handle lives in are closed, and the command-line remedy is
        // not available to a design built on attaching to a JVM somebody else
        // launched. An instrumentation agent may open them itself.
        ModuleAccess.openDesktopInternals(instrumentation);
        SwingAdapter adapter = SwingAdapter.installIfPresent(this, new LinkedHashSet<String>(toolkits));
        if (adapter != null) {
            adapter.contributeMethods(methods);
            swingAdapter = adapter;
        }
    }

    private Map<String, Object> info() {
        Map<String, Object> result = Json.newObject();
        result.put("protocol", Long.valueOf(HandshakeFile.PROTOCOL_VERSION));
        result.put("agentVersion", Agent.version());
        result.put("pid", Long.valueOf(pid));
        result.put("port", Long.valueOf(port));
        result.put("toolkits", detectToolkits());
        return result;
    }

    /**
     * Runs a read on the toolkit thread under the agent-side deadline.
     *
     * <p>Every element access an adapter adds goes through here, so the boundedness is a property
     * of the runtime and not something each adapter has to remember.
     */
    <T> T onToolkitThread(java.util.concurrent.Callable<T> task) throws RpcException {
        return ToolkitDispatcher.Calls.invokeWithDeadline(dispatcher, task, DEFAULT_CALL_DEADLINE_MS);
    }

    /**
     * Authenticates a connection and pins the version.
     *
     * <p>Both checks abort rather than degrade. The token check, because loopback TCP is open to
     * every local user. The version check, because an agent cannot be unloaded: a JVM that already
     * carries version A cannot be upgraded to B, so a mismatched client has exactly one remedy —
     * restart the application — and pretending to interoperate would only find that out later, in
     * the middle of a test (design decision 1a).
     */
    private final class HandshakeMethod implements RpcMethod {

        @Override
        public Object invoke(RpcSession session, Map<String, Object> params) throws RpcException {
            String presented = Json.asString(params.get("token"));
            if (presented == null || !constantTimeEquals(presented, session.expectedToken())) {
                throw new RpcException(RpcException.UNAUTHENTICATED, "invalid token", true);
            }
            String clientVersion = Json.asString(params.get("clientVersion"));
            String agentVersion = Agent.version();
            if (clientVersion == null || !clientVersion.equals(agentVersion)) {
                throw new RpcException(RpcException.VERSION_MISMATCH,
                        "agent version " + agentVersion + " cannot serve client version " + clientVersion
                                + " — restart the application to load the matching agent",
                        true);
            }
            session.markAuthenticated();
            return info();
        }

        @Override
        public boolean allowedBeforeHandshake() {
            return true;
        }
    }

    /** Liveness of the connection itself; per-element liveness is a different endpoint. */
    private static final class PingMethod implements RpcMethod {

        @Override
        public Object invoke(RpcSession session, Map<String, Object> params) {
            Map<String, Object> result = Json.newObject();
            result.put("pong", Boolean.TRUE);
            return result;
        }

        @Override
        public boolean allowedBeforeHandshake() {
            return false;
        }
    }

    /** Re-detects on every call, so a client sees a toolkit that appeared after the handshake. */
    private final class InfoMethod implements RpcMethod {

        @Override
        public Object invoke(RpcSession session, Map<String, Object> params) {
            refreshToolkits();
            return info();
        }

        @Override
        public boolean allowedBeforeHandshake() {
            return false;
        }
    }

    /**
     * Per-element liveness (design decision 2) — what lets a provider answer node validity
     * honestly instead of assuming a node survives as long as its process does.
     */
    private final class LivenessMethod implements RpcMethod {

        @Override
        public Object invoke(RpcSession session, Map<String, Object> params) throws RpcException {
            Object rawId = params.get("id");
            if (!(rawId instanceof Long)) {
                throw new RpcException(RpcException.INVALID_PARAMS, "'id' must be an element id");
            }
            final long id = ((Long) rawId).longValue();
            // On the toolkit thread: "still attached to a showing window" is a question only the
            // toolkit may be asked, and asking it off-thread is how observers cause the races they
            // then report as flakiness.
            Boolean live = onToolkitThread(new java.util.concurrent.Callable<Boolean>() {
                @Override
                public Boolean call() {
                    return Boolean.valueOf(registry.isLive(id));
                }
            });
            Map<String, Object> result = Json.newObject();
            result.put("live", live);
            return result;
        }

        @Override
        public boolean allowedBeforeHandshake() {
            return false;
        }
    }

    /**
     * The process facts behind the provider's {@code app:Application} node.
     *
     * <p>Toolkit-independent, so it lives here rather than in an adapter: a JVM with no UI still has a
     * process, and the answer would be the same for every toolkit anyway.
     */
    private static final class ProcessMethod implements RpcMethod {

        @Override
        public Object invoke(RpcSession session, Map<String, Object> params) {
            return ProcessFacts.collect();
        }

        @Override
        public boolean allowedBeforeHandshake() {
            return false;
        }
    }

    /** Polling counterpart of the {@code ui/generation} notification. */
    private final class GenerationMethod implements RpcMethod {

        @Override
        public Object invoke(RpcSession session, Map<String, Object> params) {
            Map<String, Object> result = Json.newObject();
            result.put("generation", Long.valueOf(generation.current()));
            return result;
        }

        @Override
        public boolean allowedBeforeHandshake() {
            return false;
        }
    }

    // ---------------------------------------------------------------- helpers

    private static String newToken() {
        byte[] bytes = new byte[32];
        new SecureRandom().nextBytes(bytes);
        StringBuilder hex = new StringBuilder(bytes.length * 2);
        for (byte b : bytes) {
            hex.append(Character.forDigit((b >> 4) & 0xF, 16));
            hex.append(Character.forDigit(b & 0xF, 16));
        }
        return hex.toString();
    }

    private static boolean constantTimeEquals(String presented, String expected) {
        return MessageDigest.isEqual(presented.getBytes(UTF_8), expected.getBytes(UTF_8));
    }
}
