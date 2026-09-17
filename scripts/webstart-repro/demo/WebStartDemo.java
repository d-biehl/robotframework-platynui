package platynui.demo;

import java.awt.BorderLayout;
import javax.swing.JButton;
import javax.swing.JFrame;
import javax.swing.JLabel;
import javax.swing.JPanel;
import javax.swing.JTextField;
import javax.swing.SwingUtilities;
import javax.swing.WindowConstants;

/**
 * A minimal Swing application, launched through Java Web Start, for reproducing what the PlatynUI
 * Java agent sees inside an OpenWebStart-launched JVM.
 *
 * <p>It also reports what the JNLP sandbox permits, because that is the question under test: whether
 * the security manager an OpenWebStart application runs under is what stops the agent from
 * publishing its handshake.
 */
public final class WebStartDemo {

    private WebStartDemo() {
    }

    public static void main(String[] args) {
        report();
        SwingUtilities.invokeLater(new Runnable() {
            @Override
            public void run() {
                JFrame frame = new JFrame("PlatynUI WebStart Demo");
                frame.setDefaultCloseOperation(WindowConstants.EXIT_ON_CLOSE);
                JPanel panel = new JPanel(new BorderLayout(8, 8));
                JLabel label = new JLabel("PlatynUI WebStart demo");
                label.setName("greeting");
                JTextField field = new JTextField("editable", 20);
                field.setName("input");
                JButton button = new JButton("Press me");
                button.setName("press");
                panel.add(label, BorderLayout.NORTH);
                panel.add(field, BorderLayout.CENTER);
                panel.add(button, BorderLayout.SOUTH);
                frame.setContentPane(panel);
                frame.setSize(420, 200);
                frame.setLocation(200, 200);
                frame.setVisible(true);
            }
        });
    }

    /** Prints the facts the agent's start-up depends on, so a failed start can be read off the log. */
    private static void report() {
        // Every one of these is permission-gated under a JNLP sandbox, so none of them
        // may take the application down — the point is to observe the sandbox, not to
        // be killed by it.
        System.out.println("[demo] pid            = " + safely(new Supplier() {
            public String get() {
                return pid();
            }
        }));
        System.out.println("[demo] java.version   = " + safely(new Supplier() {
            public String get() {
                return System.getProperty("java.version");
            }
        }));
        System.out.println("[demo] java.home      = " + safely(new Supplier() {
            public String get() {
                return System.getProperty("java.home");
            }
        }));
        System.out.println("[demo] SecurityManager= " + System.getSecurityManager());
        probe("getenv LOCALAPPDATA", new Probe() {
            @Override
            public void run() {
                System.getenv("LOCALAPPDATA");
            }
        });
        probe("write %LOCALAPPDATA%", new Probe() {
            @Override
            public void run() throws Exception {
                java.io.File dir = new java.io.File(System.getenv("LOCALAPPDATA"), "PlatynUI\\probe");
                dir.mkdirs();
                java.io.File probe = new java.io.File(dir, "probe.txt");
                java.io.FileOutputStream out = new java.io.FileOutputStream(probe);
                out.write(42);
                out.close();
                probe.delete();
            }
        });
        probe("listen on loopback", new Probe() {
            @Override
            public void run() throws Exception {
                java.net.ServerSocket socket =
                        new java.net.ServerSocket(0, 1, java.net.InetAddress.getByName("127.0.0.1"));
                socket.close();
            }
        });
        probe("addShutdownHook", new Probe() {
            @Override
            public void run() {
                Thread hook = new Thread();
                Runtime.getRuntime().addShutdownHook(hook);
                Runtime.getRuntime().removeShutdownHook(hook);
            }
        });
    }

    private interface Probe {
        void run() throws Exception;
    }

    private interface Supplier {
        String get();
    }

    private static String safely(Supplier supplier) {
        try {
            return supplier.get();
        } catch (Throwable e) {
            return "<denied: " + e.getClass().getSimpleName() + ">";
        }
    }

    private static void probe(String what, Probe probe) {
        try {
            probe.run();
            System.out.println("[demo] permitted: " + what);
        } catch (Throwable e) {
            System.out.println("[demo] DENIED   : " + what + " -> " + e);
        }
    }

    private static String pid() {
        String name = java.lang.management.ManagementFactory.getRuntimeMXBean().getName();
        int at = name.indexOf('@');
        return at > 0 ? name.substring(0, at) : name;
    }
}
