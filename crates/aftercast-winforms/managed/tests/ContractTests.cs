using System.Windows.Forms;

// Check the standalone managed contract without loading ACT or its bridge.
static void Check(bool condition) { if (!condition) throw new Exception("WinForms compatibility contract check failed"); }
var assembly = typeof(TabPage).Assembly;
var identity = assembly.GetName();
Check(identity.Name == "System.Windows.Forms" && identity.Version == new Version(4, 0, 0, 0));
Check(identity.GetPublicKeyToken()!.Length == 0);
Check(typeof(Label).Assembly == assembly && typeof(UserControl).BaseType == typeof(ContainerControl));
Check(assembly.GetReferencedAssemblies().All(reference => reference.Name is not ("Advanced Combat Tracker" or "FSharp.Core")));
foreach (var control in new Control[] { new TabPage(), new Label(), new UserControl() })
{
    using (control)
    {
        var disposedEvents = 0;
        control.Disposed += (_, _) => disposedEvents++;
        Check(!control.IsDisposed);
        control.Text = "Ready";
        Check(control.Text == "Ready");
        control.Text = null!;
        Check(control.Text == "");
        try { _ = control.Handle; throw new Exception("Unsupported native handle unexpectedly succeeded"); }
        catch (PlatformNotSupportedException error) { Check(error.Message.Contains("native handles")); }
        control.Dispose();
        control.Dispose();
        Check(control.IsDisposed && disposedEvents == 1);
    }
}
Console.WriteLine("Independent WinForms identity, text, disposal and native-handle rejection passed.");
ChartContracts.Run();
ListViewContracts.Run();
TreeViewContracts.Run();
TimerContracts.Run();
ButtonContracts.Run();
ControlModelContracts.Run();
FormStateContracts.Run();
ListControlContracts.Run();
SettingsControlContracts.Run();
