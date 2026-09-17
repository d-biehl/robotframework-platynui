import com.sun.tools.attach.VirtualMachine;

/** Attaches an arbitrary agent JAR: Attach &lt;pid&gt; &lt;jar&gt; [args] */
public class Attach {
    public static void main(String[] args) throws Exception {
        VirtualMachine vm = VirtualMachine.attach(args[0]);
        vm.loadAgent(args[1], args.length > 2 ? args[2] : null);
        vm.detach();
        System.out.println("attached " + args[1]);
    }
}
