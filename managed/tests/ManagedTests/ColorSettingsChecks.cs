using System.Windows.Forms;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class ColorSettingsChecks
{
    private static void Check(bool value,string message) { if(!value) throw new Exception("Color settings: "+message); }
    internal static void Run()
    {
        var directory=Directory.CreateTempSubdirectory("aftercast-color-settings-");
        try
        {
            using var host=new HostEnvironment(directory.FullName); var main=ActGlobals.oFormActMain!; var settings=main.ActColorSettings;
            Check(!settings.InvertColors && settings.LuminosityOffset==0m && settings.SaturationOffset==0m,"defaults");
            Check(settings.DataGridAllyText.ForeColorSetting==System.Drawing.Color.ForestGreen && settings.DataGridPersonalBackground.ForeColorSetting.ToArgb()==System.Drawing.Color.FromArgb(200,255,200).ToArgb(),"color control defaults");
            var colorEvents=new List<string>();
            ColorControl.ColorSettingEventDelegate ally=value=>colorEvents.Add("ally:"+value.ToArgb());
            ColorControl.ColorSettingEventDelegate personal=value=>colorEvents.Add("personal:"+value.ToArgb());
            settings.DataGridAllyText.ForeColorSettingChanged+=ally; settings.DataGridAllyText.ForeColorSettingChanged+=ally;
            settings.DataGridPersonalBackground.ForeColorSettingChanged+=personal;
            var custom=System.Drawing.Color.FromArgb(1,2,3); host.ApplyColorControls(custom,System.Drawing.Color.Empty);
            Check(colorEvents.SequenceEqual([$"ally:{custom.ToArgb()}",$"ally:{custom.ToArgb()}","personal:0"]),"color order and duplicates");
            colorEvents.Clear(); host.ApplyColorControls(custom,System.Drawing.Color.Empty); Check(colorEvents.Count==0,"identical color silent");
            settings.DataGridAllyText.ForeColorSettingChanged-=ally; settings.DataGridAllyText.ForeColorSettingChanged-=ally;
            settings.DataGridPersonalBackground.ForeColorSettingChanged-=personal;
            try { host.ApplyColorControls(System.Drawing.Color.FromArgb(5,1,2,3),custom); throw new Exception("transparent color accepted"); } catch(ArgumentException) {}
            Check(settings.DataGridAllyText.ForeColorSetting==custom,"invalid control update atomic");
            var nested=System.Drawing.Color.FromArgb(4,5,6); var changed=System.Drawing.Color.FromArgb(7,8,9);
            ColorControl.ColorSettingEventDelegate recursive=value=>{if(value==changed)settings.DataGridAllyText.ForeColorSetting=nested;};
            settings.DataGridAllyText.ForeColorSettingChanged+=recursive; host.ApplyColorControls(changed,System.Drawing.Color.Empty); settings.DataGridAllyText.ForeColorSettingChanged-=recursive;
            Check(settings.DataGridAllyText.ForeColorSetting==nested,"recursive color handler observes runtime state");
            ColorControl.ColorSettingEventDelegate bad=_=>throw new IOException("synthetic color handler failure"); settings.DataGridAllyText.ForeColorSettingChanged+=bad;
            try { host.ApplyColorControls(custom,custom); throw new Exception("color exception suppressed"); } catch(IOException) {}
            settings.DataGridAllyText.ForeColorSettingChanged-=bad;
            Check(settings.DataGridAllyText.ForeColorSetting==custom && settings.DataGridPersonalBackground.ForeColorSetting.IsEmpty,"color handler error leaves sequential state");


            var events=new List<string>(); var owner=Environment.CurrentManagedThreadId; object? invertSender=null,lumSender=null;
            EventHandler invert=(sender,_)=>{Check(Environment.CurrentManagedThreadId==owner && sender is CheckBox,"invert owner/sender");invertSender=sender;events.Add("invert");};
            EventHandler lum=(sender,_)=>{Check(Environment.CurrentManagedThreadId==owner && sender is NumericUpDown,"lum owner/sender");lumSender=sender;events.Add("lum");};
            EventHandler sat=(sender,_)=>{Check(Environment.CurrentManagedThreadId==owner && sender is NumericUpDown,"sat owner/sender");events.Add("sat");};
            settings.InvertColorsChanged+=invert; settings.LuminosityOffsetChanged+=lum; settings.SaturationOffsetChanged+=sat;
            settings.InvertColorsChanged+=invert;
            host.ApplyColorScalars(true,.25m,-.75m);
            Check(events.SequenceEqual(["invert","invert","lum","sat"]) && settings.InvertColors && settings.LuminosityOffset==.25m && settings.SaturationOffset==-.75m,"order, duplicate subscriptions and values");
            events.Clear();host.ApplyColorScalars(true,.25m,-.75m);Check(events.Count==0,"unchanged controls are silent");
            settings.InvertColorsChanged-=invert;events.Clear();host.ApplyColorScalars(false,.25m,-.75m);Check(events.SequenceEqual(["invert"]),"remove only one duplicate");
            settings.InvertColorsChanged-=invert;settings.LuminosityOffsetChanged-=lum;settings.SaturationOffsetChanged-=sat;
            EventHandler recurse=(sender,_)=>{var numeric=(NumericUpDown)sender!;if(numeric.Value==.1m)numeric.Value=.2m;};
            settings.LuminosityOffsetChanged+=recurse;host.ApplyColorScalars(false,.1m,-.75m);settings.LuminosityOffsetChanged-=recurse;
            Check(settings.LuminosityOffset==.2m,"reentrant control state");
            var rejectedShutdown=false; EventHandler stop=(_,_)=>{try{host.Dispose();}catch(InvalidOperationException){rejectedShutdown=true;}};
            settings.InvertColorsChanged+=stop;host.ApplyColorScalars(true,.2m,-.75m);settings.InvertColorsChanged-=stop;
            Check(rejectedShutdown && ReferenceEquals(ActGlobals.oFormActMain,main),"subscriber cannot dispose active owner");
            host.ApplyColorScalars(false,.2m,-.75m);
            EventHandler fail=(_,_)=>throw new IOException("synthetic subscriber failure");settings.InvertColorsChanged+=fail;
            try{host.ApplyColorScalars(true,.3m,.4m);throw new Exception("exception suppressed");}catch(IOException){}
            Check(settings.InvertColors && settings.LuminosityOffset==.2m && settings.SaturationOffset==-.75m,"subscriber exception leaves sequential partial state visible");
            settings.InvertColorsChanged-=fail;host.ApplyColorScalars(true,.3m,.4m);
            try{host.ApplyColorScalars(false,1.01m,0m);throw new Exception("invalid accepted");}catch(ArgumentOutOfRangeException){}
            Check(settings.InvertColors && settings.LuminosityOffset==.3m,"invalid input changes nothing");
            try{Task.Run(()=>host.ApplyColorScalars(false,0m,0m)).GetAwaiter().GetResult();throw new Exception("wrong thread accepted");}catch(InvalidOperationException){}
            var retiredAlly=settings.DataGridAllyText; colorEvents.Clear(); retiredAlly.ForeColorSettingChanged+=ally;
            events.Clear(); settings.InvertColorsChanged+=invert; settings.LuminosityOffsetChanged+=lum;
            host.Dispose();Check(((Control)invertSender!).IsDisposed && ((Control)lumSender!).IsDisposed,"owner controls disposed");
            ((CheckBox)invertSender!).Checked=false; ((NumericUpDown)lumSender!).Value=.5m; retiredAlly.ForeColorSetting=System.Drawing.Color.Red;
            Check(events.Count==0 && colorEvents.Count==0,"retired controls release subscribers");
            try{host.ApplyColorScalars(false,0m,0m);throw new Exception("disposed accepted");}catch(ObjectDisposedException){}
        }
        finally { directory.Delete(true); }
        var second=Directory.CreateTempSubdirectory("aftercast-color-dispose-");
        try
        {
            var host=new HostEnvironment(second.FullName); var settings=ActGlobals.oFormActMain!.ActColorSettings;
            Control? first=null, secondControl=null, third=null;
            settings.InvertColorsChanged+=(sender,_)=>first=(Control)sender!;
            settings.LuminosityOffsetChanged+=(sender,_)=>secondControl=(Control)sender!;
            settings.SaturationOffsetChanged+=(sender,_)=>third=(Control)sender!;
            host.ApplyColorScalars(true,.1m,.1m);
            first!.Disposed+=(_,_)=>throw new IOException("synthetic dispose error");
            try { host.Dispose(); throw new Exception("dispose error suppressed"); } catch(AggregateException error) { Check(error.ToString().Contains("synthetic dispose error"),"dispose error preserved"); }
            Check(secondControl!.IsDisposed && third!.IsDisposed && ActGlobals.oFormActMain is null,"remaining controls and globals cleaned after failure");
        }
        finally { second.Delete(true); }
        Console.WriteLine("Color scalar getters, control senders, order, reentry and disposal passed.");
    }
}
