package platynui.testapp;

import java.awt.FlowLayout;
import javax.swing.BorderFactory;
import javax.swing.ButtonGroup;
import javax.swing.JCheckBox;
import javax.swing.JComboBox;
import javax.swing.JPanel;
import javax.swing.JProgressBar;
import javax.swing.JRadioButton;
import javax.swing.JSlider;
import javax.swing.JSpinner;
import javax.swing.SpinnerNumberModel;

/**
 * Stage 2 controls: checkbox, radio group, combo box, slider, spinner, progress bar.
 *
 * <p>Fixed accessible names (never change): {@code stage2-panel}, {@code stage2-checkbox},
 * {@code stage2-radio-a}, {@code stage2-radio-b}, {@code stage2-combo}, {@code stage2-slider},
 * {@code stage2-spinner}, {@code stage2-progress}.
 */
final class Stage2Panel extends JPanel {

    private static final long serialVersionUID = 1L;

    Stage2Panel() {
        super(new FlowLayout(FlowLayout.LEFT, 8, 8));
        getAccessibleContext().setAccessibleName("stage2-panel");
        setBorder(BorderFactory.createTitledBorder("Stage 2"));

        JCheckBox checkBox = new JCheckBox("Enable option");
        checkBox.getAccessibleContext().setAccessibleName("stage2-checkbox");

        JRadioButton radioA = new JRadioButton("Option A", true);
        radioA.getAccessibleContext().setAccessibleName("stage2-radio-a");
        JRadioButton radioB = new JRadioButton("Option B");
        radioB.getAccessibleContext().setAccessibleName("stage2-radio-b");
        ButtonGroup radioGroup = new ButtonGroup();
        radioGroup.add(radioA);
        radioGroup.add(radioB);

        JComboBox<String> comboBox = new JComboBox<String>(new String[] {"Alpha", "Beta", "Gamma"});
        comboBox.getAccessibleContext().setAccessibleName("stage2-combo");

        JSlider slider = new JSlider(0, 100, 50);
        slider.getAccessibleContext().setAccessibleName("stage2-slider");

        JSpinner spinner = new JSpinner(new SpinnerNumberModel(5, 0, 100, 1));
        spinner.getAccessibleContext().setAccessibleName("stage2-spinner");

        JProgressBar progressBar = new JProgressBar(0, 100);
        progressBar.setValue(30);
        progressBar.getAccessibleContext().setAccessibleName("stage2-progress");

        add(checkBox);
        add(radioA);
        add(radioB);
        add(comboBox);
        add(slider);
        add(spinner);
        add(progressBar);
    }
}
