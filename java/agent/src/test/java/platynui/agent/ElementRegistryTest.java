package platynui.agent;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

/**
 * The registry's promises are the ones a provider's {@code UiNode::is_valid} rests on, so they are
 * worth pinning: stable ids, identity rather than equality, no strong references, and an honest
 * liveness answer.
 */
class ElementRegistryTest {

    /** A component stand-in whose {@code equals} deliberately lies, like a value-ish list cell. */
    private static final class EqualToEverything {
        @Override
        public boolean equals(Object other) {
            return other instanceof EqualToEverything;
        }

        @Override
        public int hashCode() {
            return 42;
        }
    }

    @Test
    void the_same_object_always_gets_the_same_id() {
        ElementRegistry registry = new ElementRegistry();
        Object element = new Object();
        long first = registry.idFor(element);
        long second = registry.idFor(element);
        assertEquals(first, second, "a re-enumerated component must keep its identity");
        assertSame(element, registry.resolve(first));
    }

    @Test
    void distinct_objects_get_distinct_ids_even_when_they_are_equal() {
        // Two table cells can compare equal and still be different nodes. An
        // equality-keyed registry would merge them and hand out one id for both.
        ElementRegistry registry = new ElementRegistry();
        EqualToEverything a = new EqualToEverything();
        EqualToEverything b = new EqualToEverything();
        assertEquals(a, b, "the fixture must actually be equality-confusable");
        assertNotEquals(registry.idFor(a), registry.idFor(b));
    }

    @Test
    void an_unknown_id_resolves_to_nothing_and_is_not_live() {
        ElementRegistry registry = new ElementRegistry();
        assertNull(registry.resolve(999L));
        assertFalse(registry.isLive(999L));
    }

    @Test
    void a_null_element_is_rejected_rather_than_given_an_id() {
        ElementRegistry registry = new ElementRegistry();
        assertThrows(IllegalArgumentException.class, () -> registry.idFor(null));
    }

    /**
     * The load-bearing property: the registry must not keep the target's UI alive. A strong-ref
     * cache would pin every component a long test run ever touched.
     */
    @Test
    void collected_elements_drop_out_of_the_registry() throws InterruptedException {
        ElementRegistry registry = new ElementRegistry();
        Object survivor = new Object();
        long survivorId = registry.idFor(survivor);

        List<Long> doomedIds = new ArrayList<>();
        for (int i = 0; i < 200; i++) {
            doomedIds.add(registry.idFor(new Object()));
        }
        assertEquals(201, registry.size());

        // Nothing references the 200 objects any more. GC is not on demand, so
        // this asks repeatedly rather than assuming one call suffices.
        long deadline = System.currentTimeMillis() + 10_000L;
        while (System.currentTimeMillis() < deadline && registry.size() > 1) {
            System.gc();
            Thread.sleep(50L);
        }

        assertEquals(1, registry.size(), "only the strongly-referenced element may survive");
        assertSame(survivor, registry.resolve(survivorId), "a live element must not be swept");
        for (Long id : doomedIds) {
            assertFalse(registry.isLive(id.longValue()), "a collected element must report not-live");
        }
    }

    /**
     * "Not collected" is necessary but not sufficient — a detached component is still strongly
     * held by whoever detached it, so the toolkit gets the last word.
     */
    @Test
    void the_toolkit_gets_the_last_word_on_liveness() {
        ElementRegistry registry = new ElementRegistry();
        Object element = new Object();
        long id = registry.idFor(element);
        assertTrue(registry.isLive(id), "reachable is the neutral default");

        registry.setLivenessCheck(candidate -> false);
        assertFalse(registry.isLive(id), "an adapter must be able to report a detached element as dead");

        registry.setLivenessCheck(null);
        assertTrue(registry.isLive(id), "clearing the check restores the neutral default");
    }
}
