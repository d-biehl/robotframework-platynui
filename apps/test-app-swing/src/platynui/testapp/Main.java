package platynui.testapp;

import java.io.PrintStream;
import java.security.AccessController;
import java.security.AllPermission;
import javax.swing.BoxLayout;
import javax.swing.JFrame;
import javax.swing.JMenu;
import javax.swing.JMenuBar;
import javax.swing.JMenuItem;
import javax.swing.JPanel;
import javax.swing.SwingUtilities;
import javax.swing.Timer;

/**
 * PlatynUI Swing test application.
 *
 * <p>An accessibility fixture for the Java Access Bridge (JAB) provider work. Every interactive
 * control carries an explicit, unique accessible name: JAB exposes no AutomationId equivalent, so
 * the accessible name is the locator anchor for all downstream tests. Existing accessible names
 * must never change when the app grows (see README.md).
 */
public final class Main {

    static final String DEFAULT_TITLE = "PlatynUI Swing TestApp";

    /** Strong reference to the launcher furniture of {@code --app-context}; see there. */
    @SuppressWarnings("unused")
    private static JFrame launcherFurniture;

    private Main() {
    }

    public static void main(String[] args) {
        final Options options = Options.parse(args);
        if (options.requireSecurityManager) {
            requireSandboxShape();
        }
        if (options.appContext) {
            showInNewAppContext(options);
        } else {
            SwingUtilities.invokeLater(() -> createAndShow(options));
        }
    }

    /**
     * Builds the UI inside a second AWT {@code AppContext} — the shape a Java Web Start or applet
     * runtime produces, where each hosted application gets its own (OpenSpec
     * {@code java-agent-web-start}).
     *
     * <p>Why a fixture needs it: {@code Window.getWindows()} and {@code EventQueue.invokeLater}
     * both resolve against the <em>calling thread's</em> context. An observer whose threads live in
     * the system context therefore sees no windows and posts work to a queue that will never run
     * it — while every single-context test it has keeps passing. Only a target in this shape can
     * tell the two apart.
     *
     * <p>The UI is the default one, unchanged: same components, same accessible names. The context
     * is the variable, not the content.
     */
    private static void showInNewAppContext(Options options) {
        // The launcher's world, built first and deliberately: without it the new context would be
        // the JVM's ONLY one, since nothing else here touches AWT — measured, and a materially
        // easier target than Web Start, where the runtime already owns a context before the
        // application gets its own. This frame is the launcher's furniture (OWS has a
        // SharedOwnerFrame and its download dialogs): it belongs to a different application than
        // the one under test, and it is never shown, which is exactly why it must stay out of the
        // window list an observer reports.
        JFrame furniture = new JFrame("PlatynUI TestApp launcher");
        furniture.getAccessibleContext().setAccessibleName("launcher-furniture");
        furniture.setSize(120, 80);
        if (options.companionWindow) {
            // A *showing* window in the launcher's world, so the JVM has two toolkit worlds that
            // both have something to read. Without it the launcher's world is invisible by
            // construction and "one world wedged, the other still answering" cannot be observed.
            furniture.setTitle(options.title + " companion");
            furniture.getAccessibleContext().setAccessibleName("companion-window");
            furniture.setLocation(40, 40);
            furniture.setVisible(true);
        }

        ThreadGroup group = new ThreadGroup("PlatynUI TestApp AppContext");
        Thread thread = new Thread(group, () -> {
            createNewAppContext();
            SwingUtilities.invokeLater(() -> createAndShow(options));
        }, "platynui-testapp-appcontext");
        thread.start();
        // Kept reachable for the life of the process: a collected frame would drop out of its
        // context's weak window list and take the coverage of "furniture stays out" with it.
        launcherFurniture = furniture;
    }

    /**
     * Creates an {@code AppContext} for the calling thread's group, reflectively: {@code
     * sun.awt.SunToolkit} is a JDK internal and there is no public API for this — creating one per
     * hosted application is exactly what the applet and Web Start runtimes do.
     *
     * <p>Failure exits instead of degrading. A fixture that quietly kept a single context would
     * still show its window and still satisfy every assertion that does not depend on the second
     * one — which is the whole of what this mode exists to provide.
     */
    private static void createNewAppContext() {
        try {
            Class.forName("sun.awt.SunToolkit").getMethod("createNewAppContext").invoke(null);
        } catch (ReflectiveOperationException | RuntimeException e) {
            System.err.println("error: --app-context could not create a second AppContext: " + e);
            System.err.println("  sun.awt is not exported on JDK 9+; launch the fixture with");
            System.err.println("  --add-exports java.desktop/sun.awt=ALL-UNNAMED");
            System.err.println("  (JDK_JAVA_OPTIONS carries it on 9+ and is ignored by Java 8).");
            System.exit(3);
        }
    }

    /**
     * Asserts that the sandbox this fixture was asked to run in actually applies: a security
     * manager is installed, and the application's own code holds all permissions — the shape a
     * signed {@code <all-permissions/>} JNLP produces, where the application is trusted and
     * anything else in the JVM (an agent appended to the system class path included) is not.
     *
     * <p>Checked rather than assumed, because the failure is silent in both directions. A policy
     * file whose {@code codeBase} matches nothing loads without any error and leaves the fixture
     * sandboxed too — a harsher and different shape, in which "the agent works anyway" would be
     * asserted against a target that is itself crippled.
     */
    private static void requireSandboxShape() {
        if (System.getSecurityManager() == null) {
            System.err.println("error: --require-security-manager, but no security manager is installed");
            System.err.println("  launch with -Djava.security.manager -Djava.security.policy=<policy file>");
            System.exit(4);
        }
        try {
            AccessController.checkPermission(new AllPermission());
        } catch (SecurityException e) {
            System.err.println("error: the fixture's own code is sandboxed, so the policy did not apply to it");
            System.err.println("  denied: " + e);
            // Naming the actual code source here would be the useful thing to print, and it is
            // exactly what this state forbids: getProtectionDomain() needs a RuntimePermission
            // the sandbox does not grant, so reading it would replace this diagnostic with a
            // stack trace about reading it.
            System.err.println("  compare -Dplatynui.fixture.classes with the -cp entry: the policy's");
            System.err.println("  codeBase must match, as a URL path with forward slashes.");
            System.exit(4);
        }
    }

    private static void createAndShow(Options options) {
        JFrame frame = new JFrame(options.title);
        // The frame's accessible name deliberately tracks the title (Swing's default behavior),
        // so --title changes the window title and the accessible name together.
        frame.setDefaultCloseOperation(JFrame.EXIT_ON_CLOSE);
        frame.setJMenuBar(createMenuBar());

        JPanel content = new JPanel();
        content.setLayout(new BoxLayout(content, BoxLayout.Y_AXIS));
        content.getAccessibleContext().setAccessibleName("main-content");
        content.add(new Stage1Panel());
        content.add(new Stage2Panel());
        content.add(new TablePanel());
        frame.setContentPane(content);

        frame.pack();
        frame.setLocationByPlatform(true);
        frame.setVisible(true);

        if (options.autoCloseSeconds > 0) {
            Timer timer = new Timer(options.autoCloseSeconds * 1000, e -> System.exit(0));
            timer.setRepeats(false);
            timer.start();
        }
        if (options.wedgeForSeconds > 0) {
            scheduleWedge(options);
        }
    }

    /**
     * Blocks <em>this</em> window's event queue for a while — the one an observer has to survive
     * per toolkit world rather than per JVM.
     *
     * <p>A wedged queue is not a contrived state: a long application callback, a modal native
     * dialog or a deadlock produce it, and the agent's promise is that a call into it is abandoned
     * at its deadline instead of pinning the handler. With two worlds the promise gets sharper —
     * the other world must keep answering — and that is only observable if exactly one of them can
     * be stopped.
     */
    private static void scheduleWedge(Options options) {
        final long blockMs = options.wedgeForSeconds * 1000L;
        Timer timer = new Timer(Math.max(options.wedgeAfterSeconds, 1) * 1000, e -> {
            System.err.println("[TestApp] wedging this event queue for " + blockMs + "ms");
            long until = System.currentTimeMillis() + blockMs;
            // Busy-ish wait rather than Thread.sleep in one go: interruption would end the block
            // early and quietly turn a timeout test into a flake.
            while (System.currentTimeMillis() < until) {
                try {
                    Thread.sleep(50L);
                } catch (InterruptedException interrupted) {
                    Thread.currentThread().interrupt();
                }
            }
            System.err.println("[TestApp] event queue released");
        });
        timer.setRepeats(false);
        timer.start();
    }

    private static JMenuBar createMenuBar() {
        JMenuBar menuBar = new JMenuBar();
        menuBar.getAccessibleContext().setAccessibleName("main-menubar");

        JMenu fileMenu = new JMenu("File");
        fileMenu.getAccessibleContext().setAccessibleName("menu-file");
        JMenuItem exitItem = new JMenuItem("Exit");
        exitItem.getAccessibleContext().setAccessibleName("menu-file-exit");
        exitItem.addActionListener(e -> System.exit(0));
        fileMenu.add(exitItem);

        JMenu helpMenu = new JMenu("Help");
        helpMenu.getAccessibleContext().setAccessibleName("menu-help");
        JMenuItem aboutItem = new JMenuItem("About");
        aboutItem.getAccessibleContext().setAccessibleName("menu-help-about");
        aboutItem.addActionListener(
                e -> System.out.println(DEFAULT_TITLE + " - accessibility fixture for PlatynUI"));
        helpMenu.add(aboutItem);

        menuBar.add(fileMenu);
        menuBar.add(helpMenu);
        return menuBar;
    }

    /** Hand-rolled CLI options; mirrors the conventions of the Qt/egui test apps. */
    static final class Options {

        String title = DEFAULT_TITLE;
        int autoCloseSeconds;
        int dialogs; // reserved for stage 4; accepted, currently a no-op
        boolean openModal; // reserved for stage 4; accepted, currently a no-op
        boolean appContext; // build the UI in a second AWT AppContext (Web Start shape)
        boolean requireSecurityManager; // assert the sandbox applied before showing anything
        boolean companionWindow; // also SHOW the launcher's window, so both worlds are readable
        int wedgeAfterSeconds = 5; // when the wedge starts, counted from the UI being up
        int wedgeForSeconds; // how long this window's event queue stops processing (0: never)

        static Options parse(String[] args) {
            Options options = new Options();
            for (int i = 0; i < args.length; i++) {
                String arg = args[i];
                if ("--title".equals(arg)) {
                    options.title = requireValue(args, ++i, arg);
                } else if ("--auto-close".equals(arg)) {
                    options.autoCloseSeconds = requireInt(args, ++i, arg);
                } else if ("--dialogs".equals(arg)) {
                    options.dialogs = requireInt(args, ++i, arg);
                } else if ("--open-modal".equals(arg)) {
                    options.openModal = true;
                } else if ("--app-context".equals(arg)) {
                    options.appContext = true;
                } else if ("--require-security-manager".equals(arg)) {
                    options.requireSecurityManager = true;
                } else if ("--companion-window".equals(arg)) {
                    options.companionWindow = true;
                } else if ("--wedge-after".equals(arg)) {
                    options.wedgeAfterSeconds = requireInt(args, ++i, arg);
                } else if ("--wedge-for".equals(arg)) {
                    options.wedgeForSeconds = requireInt(args, ++i, arg);
                } else if ("--help".equals(arg) || "-h".equals(arg)) {
                    printUsage(System.out);
                    System.exit(0);
                } else {
                    System.err.println("error: unknown argument: " + arg);
                    printUsage(System.err);
                    System.exit(2);
                }
            }
            return options;
        }

        private static String requireValue(String[] args, int index, String arg) {
            if (index >= args.length) {
                System.err.println("error: missing value for " + arg);
                printUsage(System.err);
                System.exit(2);
                throw new AssertionError("unreachable");
            }
            return args[index];
        }

        private static int requireInt(String[] args, int index, String arg) {
            String value = requireValue(args, index, arg);
            try {
                return Integer.parseInt(value);
            } catch (NumberFormatException e) {
                System.err.println("error: value for " + arg + " must be an integer, got: " + value);
                printUsage(System.err);
                System.exit(2);
                throw new AssertionError("unreachable");
            }
        }

        private static void printUsage(PrintStream out) {
            out.println("usage: platynui.testapp.Main [options]");
            out.println();
            out.println("  --title <text>          window title (default: \"" + DEFAULT_TITLE + "\")");
            out.println("  --auto-close <seconds>  exit with code 0 after <seconds> (for CI)");
            out.println("  --dialogs <n>           reserved for stage 4 (accepted, currently a no-op)");
            out.println("  --open-modal            reserved for stage 4 (accepted, currently a no-op)");
            out.println("  --app-context           build the UI in a second AWT AppContext (the Web");
            out.println("                          Start/applet shape); needs --add-exports");
            out.println("                          java.desktop/sun.awt=ALL-UNNAMED on JDK 9+");
            out.println("  --require-security-manager");
            out.println("                          exit unless a security manager is installed AND");
            out.println("                          this code holds all permissions");
            out.println("  --companion-window      with --app-context, also show the launcher's");
            out.println("                          window, so both toolkit worlds are readable");
            out.println("  --wedge-after <seconds> when --wedge-for starts (default: 5)");
            out.println("  --wedge-for <seconds>   block this window's event queue for <seconds>");
            out.println("  --help, -h              show this help and exit");
        }
    }
}
