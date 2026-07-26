package platynui.agent;

import java.util.Map;

/**
 * The coordinate contract on the wire: <strong>physical desktop pixels</strong> (design
 * decision 3).
 *
 * <p>The conversion happens here, inside the JVM, because this is where the toolkit's own scaling
 * knowledge lives — a Swing component knows its {@code GraphicsConfiguration}, a JavaFX node its
 * screen. Doing it on the provider side would mean a calibration heuristic guessing at a scale
 * factor the target already knows, which is exactly what JAB forced on us and exactly what this
 * contract removes.
 *
 * <p>So the wire carries one unambiguous space and the provider stays dumb. <em>How</em> a given
 * toolkit converts is the adapter's business; this class fixes only the shape and the meaning.
 *
 * <p>Origin is the virtual desktop's top-left, y grows downwards, and a rectangle is
 * {@code {x, y, width, height}} — matching what PlatynUI's own bounding-rectangle attribute means,
 * so no consumer has to translate twice.
 */
final class Geometry {

    private Geometry() {
        // Static helper.
    }

    /**
     * Builds a wire rectangle from values that are <strong>already</strong> physical desktop
     * pixels.
     *
     * @param x left edge in physical desktop pixels
     * @param y top edge in physical desktop pixels
     * @param width width in physical desktop pixels
     * @param height height in physical desktop pixels
     */
    static Map<String, Object> rect(double x, double y, double width, double height) {
        Map<String, Object> rect = Json.newObject();
        rect.put("x", Double.valueOf(x));
        rect.put("y", Double.valueOf(y));
        rect.put("width", Double.valueOf(width));
        rect.put("height", Double.valueOf(height));
        return rect;
    }

    /** Builds a wire point from values that are already physical desktop pixels. */
    static Map<String, Object> point(double x, double y) {
        Map<String, Object> point = Json.newObject();
        point.put("x", Double.valueOf(x));
        point.put("y", Double.valueOf(y));
        return point;
    }
}
