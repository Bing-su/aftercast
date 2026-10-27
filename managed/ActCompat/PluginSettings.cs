using System.Windows.Forms;

namespace Advanced_Combat_Tracker;

public partial class FormActMain
{
    private Form? pluginSettingsWindow;

    internal bool HasPluginSettings(TabPage tab) => OperatingSystem.IsWindows() && NativePageAttached(tab);
    private bool NativePageAttached(TabPage tab) => !tab.IsDisposed && pluginTabs is TabControl tabs &&
        !tabs.IsDisposed && tabs.TabPages.Contains(tab);

    internal void ShowPluginSettings(TabPage tab)
    {
        if (Environment.CurrentManagedThreadId != ownerThread) throw new InvalidOperationException("Plugin settings require the owner thread");
        if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("Native plugin settings require Windows");
        if (!NativePageAttached(tab)) throw new InvalidOperationException("The plugin has no attached settings page");
        ShowNativePluginSettings(tab);
    }

    private void ShowNativePluginSettings(TabPage tab)
    {
        var tabs = (TabControl)pluginTabs!;
        if (pluginSettingsWindow is null || pluginSettingsWindow.IsDisposed)
        {
            var window = new Form { Text = "aftercast — Plugin settings", Width = 900, Height = 650 };
            try
            {
                // User close hides the shared surface; only Host disposal owns plugin/control teardown.
                window.FormClosing += (_, args) =>
                {
                    if (args.CloseReason != CloseReason.UserClosing) return;
                    args.Cancel = true;
                    window.Hide();
                };
                tabs.Dock = DockStyle.Fill;
                window.Controls.Add(tabs);
                pluginSettingsWindow = window;
            }
            catch { window.Dispose(); throw; }
        }
        tabs.SelectedTab = tab;
        var current = pluginSettingsWindow;
        if (current.WindowState == FormWindowState.Minimized) current.WindowState = FormWindowState.Normal;
        current.Show();
        current.BringToFront();
        current.Activate();
    }

    internal void DisposePluginSettingsWindow()
    {
        var window = pluginSettingsWindow;
        pluginSettingsWindow = null;
        window?.Dispose();
    }
}
