package platynui.testapp;

import java.awt.FlowLayout;
import javax.swing.BorderFactory;
import javax.swing.JButton;
import javax.swing.JLabel;
import javax.swing.JPanel;
import javax.swing.JTextField;

/**
 * Stage 1 controls: push button, single-line text field, and the click-observable status label.
 *
 * <p>Fixed accessible names (never change): {@code stage1-panel}, {@code stage1-button},
 * {@code stage1-textfield}, {@code stage1-status-clicks-<n>} (the counter suffix is the click
 * observable; it starts at {@code clicks-0}).
 */
final class Stage1Panel extends JPanel {

    private static final long serialVersionUID = 1L;

    private int clicks;

    Stage1Panel() {
        super(new FlowLayout(FlowLayout.LEFT, 8, 8));
        getAccessibleContext().setAccessibleName("stage1-panel");
        setBorder(BorderFactory.createTitledBorder("Stage 1"));

        JButton button = new JButton("Click me");
        button.getAccessibleContext().setAccessibleName("stage1-button");

        JTextField textField = new JTextField(16);
        textField.getAccessibleContext().setAccessibleName("stage1-textfield");

        final JLabel status = new JLabel("clicks-0");
        status.getAccessibleContext().setAccessibleName("stage1-status-clicks-0");

        button.addActionListener(e -> {
            clicks++;
            String observable = "clicks-" + clicks;
            status.setText(observable);
            status.getAccessibleContext().setAccessibleName("stage1-status-" + observable);
        });

        add(button);
        add(textField);
        add(status);
    }
}
