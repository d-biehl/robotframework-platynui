package platynui.agent;

import java.lang.ref.Reference;
import java.lang.ref.ReferenceQueue;
import java.lang.ref.WeakReference;
import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Agent-assigned identities for the objects handed out over the wire.
 *
 * <p>Neither of the obvious shortcuts works (design decision 2). {@code identityHashCode} collides
 * and is reused after a collection, so an id could silently start meaning a different component. A
 * strong-reference cache would leak every component the automation ever touched — in a long test
 * run against a busy application, that is the whole UI history pinned in the target's heap.
 *
 * <p>So: monotonic ids, <strong>weak</strong> references, and an identity-based reverse map so the
 * same object always gets the same id back. Weakness is what makes {@link #isLive} an honest
 * answer, which is what lets an adapter implement {@code UiNode::is_valid} honestly — and consumers
 * rely on that (the Robot Framework library keeps a scoped root exactly as long as it reports
 * valid).
 *
 * <p>Identity, not {@code equals}: two distinct list cells can be equal to each other and must
 * still be distinguishable.
 */
final class ElementRegistry {

    /**
     * Decides whether a resolved object still counts as live.
     *
     * <p>"Not garbage collected" is necessary but not sufficient: a component detached from its
     * window is still strongly held by whoever detached it. Whether it is still attached to a
     * showing window is a toolkit question, so the adapter supplies this; the neutral default
     * answers what it can, which is reachability.
     */
    interface LivenessCheck {
        boolean isLive(Object element);
    }

    /** The default: an object we can still resolve counts as live. */
    static final LivenessCheck REACHABLE = new LivenessCheck() {
        @Override
        public boolean isLive(Object element) {
            return element != null;
        }
    };

    private final AtomicLong nextId = new AtomicLong();
    private final ReferenceQueue<Object> collected = new ReferenceQueue<Object>();
    private final Map<Long, ElementReference> byId = new HashMap<Long, ElementReference>();
    private final Map<IdentityKey, Long> byIdentity = new HashMap<IdentityKey, Long>();

    private volatile LivenessCheck livenessCheck = REACHABLE;

    void setLivenessCheck(LivenessCheck check) {
        livenessCheck = check == null ? REACHABLE : check;
    }

    /** The id for {@code element}, assigning one on first sight. */
    synchronized long idFor(Object element) {
        if (element == null) {
            throw new IllegalArgumentException("cannot register a null element");
        }
        sweep();
        IdentityKey key = new IdentityKey(element);
        Long existing = byIdentity.get(key);
        if (existing != null) {
            return existing.longValue();
        }
        long id = nextId.incrementAndGet();
        ElementReference reference = new ElementReference(element, collected, id, key);
        byId.put(Long.valueOf(id), reference);
        byIdentity.put(key, Long.valueOf(id));
        return id;
    }

    /** The object behind an id, or {@code null} if it is gone or was never registered. */
    synchronized Object resolve(long id) {
        sweep();
        ElementReference reference = byId.get(Long.valueOf(id));
        return reference == null ? null : reference.get();
    }

    /**
     * The cheap liveness answer (design decision 2).
     *
     * <p>Deliberately per-element and O(1): a client asking "is this node still valid?" must not
     * cost a tree walk, because it asks on every scoped-root access.
     */
    boolean isLive(long id) {
        Object element = resolve(id);
        return element != null && livenessCheck.isLive(element);
    }

    /** How many ids are currently registered; for diagnostics and tests. */
    synchronized int size() {
        sweep();
        return byId.size();
    }

    /** Drops the bookkeeping for objects the collector has taken. */
    private void sweep() {
        Reference<?> dead;
        while ((dead = collected.poll()) != null) {
            if (dead instanceof ElementReference) {
                ElementReference reference = (ElementReference) dead;
                byId.remove(Long.valueOf(reference.id));
                byIdentity.remove(reference.key);
            }
        }
    }

    /** A weak reference that remembers which entries to remove once it clears. */
    private static final class ElementReference extends WeakReference<Object> {

        private final long id;
        private final IdentityKey key;

        ElementReference(Object referent, ReferenceQueue<Object> queue, long id, IdentityKey key) {
            super(referent, queue);
            this.id = id;
            this.key = key;
        }
    }

    /**
     * Identity-based map key that does not itself keep the element alive.
     *
     * <p>The hash is captured at construction: {@code System.identityHashCode} of a collected
     * object cannot be recomputed, and a key whose hash changes would be unfindable in the map.
     */
    private static final class IdentityKey {

        private final WeakReference<Object> element;
        private final int hash;

        IdentityKey(Object element) {
            this.element = new WeakReference<Object>(element);
            this.hash = System.identityHashCode(element);
        }

        @Override
        public int hashCode() {
            return hash;
        }

        @Override
        public boolean equals(Object other) {
            if (this == other) {
                return true;
            }
            if (!(other instanceof IdentityKey)) {
                return false;
            }
            IdentityKey that = (IdentityKey) other;
            Object mine = element.get();
            // Two cleared keys are never equal: they stood for different objects, and treating
            // them as one would merge unrelated entries during a sweep.
            return mine != null && mine == that.element.get();
        }
    }
}
