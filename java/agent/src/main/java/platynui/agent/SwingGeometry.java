package platynui.agent;

import java.awt.Component;
import java.awt.GraphicsConfiguration;
import java.awt.Point;
import java.awt.Rectangle;
import java.awt.geom.AffineTransform;
import java.util.Map;

/**
 * AWT coordinates &rarr; the wire's physical desktop pixels (design decision 3).
 *
 * <p>The conversion is one multiplication, and that is worth explaining, because the obvious
 * implementation is more complicated and wrong.
 *
 * <p>On <strong>Java 8</strong> the JVM is DPI-unaware, so AWT coordinates already <em>are</em>
 * device pixels and {@link GraphicsConfiguration#getDefaultTransform()} is the identity — the same
 * code path needs no special case.
 *
 * <p>On <strong>Java 9+</strong> AWT reports user-space coordinates, and the tempting formula is
 * "translate relative to the screen's origin, scale, translate back into device space". It cancels:
 * the JDK derives a monitor's user-space bounds by dividing its device bounds by <em>that
 * monitor's</em> scale, so the device origin is {@code userOrigin * scale} and
 *
 * <pre>{@code
 * device = userOrigin * scale + (user - userOrigin) * scale == user * scale
 * }</pre>
 *
 * <p>which holds per monitor, mixed scale factors included. What does not hold is any assumption
 * that user space is contiguous across monitors — on a mixed-DPI desktop the JDK leaves gaps in it.
 * That is precisely why the scale is read from the component's <em>own</em>
 * {@code GraphicsConfiguration} rather than from the default screen device.
 */
final class SwingGeometry {

    private SwingGeometry() {
        // Static helper.
    }

    /**
     * Whether a rectangle encloses anything.
     *
     * <p>An empty rectangle is reported as <strong>no bounds at all</strong>, not as a rectangle at
     * the origin. A menu item whose popup has never been shown has no layout yet, and its accessible
     * component answers {@code 0x0} at its owner's corner — publishing that would tell the pointer to
     * click the menu bar, and would let the Element capability resolve on something that has no place
     * on screen. Absent is the honest answer, and consumers already handle it.
     */
    private static boolean hasArea(int width, int height) {
        return width > 0 && height > 0;
    }

    /** Horizontal/vertical device scale of the screen a component is on; {@code 1.0} when unknown. */
    static double[] scaleOf(Component component) {
        GraphicsConfiguration configuration = component == null ? null : component.getGraphicsConfiguration();
        if (configuration == null) {
            return new double[] {1.0d, 1.0d};
        }
        AffineTransform transform = configuration.getDefaultTransform();
        double scaleX = transform.getScaleX();
        double scaleY = transform.getScaleY();
        // A degenerate or flipped transform would silently produce mirrored
        // rectangles; fall back rather than emit bounds nothing can use.
        if (!(scaleX > 0.0d) || !(scaleY > 0.0d)) {
            return new double[] {1.0d, 1.0d};
        }
        return new double[] {scaleX, scaleY};
    }

    /**
     * The component's on-screen rectangle in physical desktop pixels.
     *
     * @return the wire rectangle, or {@code null} when the component is not on screen — absent
     *     bounds are an honest answer, an all-zero rectangle is not
     */
    static Map<String, Object> boundsOf(Component component) {
        if (component == null || !component.isShowing() || !hasArea(component.getWidth(), component.getHeight())) {
            return null;
        }
        Point location;
        try {
            location = component.getLocationOnScreen();
        } catch (RuntimeException e) {
            // `IllegalComponentStateException`: the component stopped showing
            // between the check above and this read. The caller gets "no bounds"
            // rather than an RPC failure.
            return null;
        }
        double[] scale = scaleOf(component);
        return Geometry.rect(
                location.x * scale[0],
                location.y * scale[1],
                component.getWidth() * scale[0],
                component.getHeight() * scale[1]);
    }

    /**
     * A rectangle expressed in {@code component}'s own coordinate space (what
     * {@code JTable.getCellRect} and friends return), converted to physical desktop pixels.
     *
     * @return the wire rectangle, or {@code null} when the component is not on screen
     */
    static Map<String, Object> boundsWithin(Component component, Rectangle local) {
        if (component == null || local == null || !component.isShowing() || !hasArea(local.width, local.height)) {
            return null;
        }
        Point origin;
        try {
            origin = component.getLocationOnScreen();
        } catch (RuntimeException e) {
            return null;
        }
        double[] scale = scaleOf(component);
        return Geometry.rect(
                (origin.x + local.x) * scale[0],
                (origin.y + local.y) * scale[1],
                local.width * scale[0],
                local.height * scale[1]);
    }

    /**
     * A physical desktop point back into {@code component}'s local coordinate space.
     *
     * <p>The inverse direction, needed for hit-testing: the host asks about a desktop pixel, and
     * {@code SwingUtilities.getDeepestComponentAt} wants user-space coordinates relative to the
     * window.
     *
     * @return the local point, or {@code null} when the component is not on screen
     */
    static Point toLocal(Component component, double deviceX, double deviceY) {
        if (component == null || !component.isShowing()) {
            return null;
        }
        Point origin;
        try {
            origin = component.getLocationOnScreen();
        } catch (RuntimeException e) {
            return null;
        }
        double[] scale = scaleOf(component);
        double userX = deviceX / scale[0];
        double userY = deviceY / scale[1];
        return new Point((int) Math.round(userX - origin.x), (int) Math.round(userY - origin.y));
    }
}
