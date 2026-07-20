package platynui.testapp;

import java.io.PrintStream;
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

    private Main() {
    }

    public static void main(String[] args) {
        final Options options = Options.parse(args);
        SwingUtilities.invokeLater(() -> createAndShow(options));
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
            out.println("  --help, -h              show this help and exit");
        }
    }
}
