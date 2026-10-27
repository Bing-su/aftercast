using System.Xml;
using System.Text;
using System.Globalization;
using Advanced_Combat_Tracker;
using Aftercast.ActCompat;

internal static class TimerManagementChecks
{
    private static void Check(bool value,string message) { if (!value) throw new InvalidOperationException(message); }
    private static void Reject(Action action)
    {
        try { action(); }
        catch (Exception error) when (error is ArgumentException or InvalidOperationException or IOException or InvalidDataException or XmlException) { return; }
        throw new Exception("Expected timer operation rejection");
    }
    internal static void Run()
    {
        var directory = Directory.CreateTempSubdirectory("aftercast-timer-management-");
        try
        {
            RoundTripAndRuntime(Directory.CreateDirectory(Path.Combine(directory.FullName,"live")).FullName);
            var culture = CultureInfo.CurrentCulture;
            try
            {
                CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("en-US");
                Failures(Directory.CreateDirectory(Path.Combine(directory.FullName,"failure")).FullName);
            }
            finally { CultureInfo.CurrentCulture = culture; }
            LegacyValues(Directory.CreateDirectory(Path.Combine(directory.FullName,"legacy")).FullName);
            StartupFailure(Directory.CreateDirectory(Path.Combine(directory.FullName,"startup")).FullName);
            Console.WriteLine("Timer definition XML, atomic edits/failures, stale/colliding keys, live-frame preservation and restart checks passed.");
        }
        finally { directory.Delete(true); }
    }
    private static TimerDefinition Definition() => new()
    {
        Name = "Spéll<&\"", Category = "Boss Zone", TimerValue = 20, WarningValue = 5, RemoveValue = -10,
        OnlyMasterTicks = true, RestrictToCategory = true, RestrictToMe = true, AbsoluteTiming = false,
        Panel1Display = false, Panel2Display = true, FillColorArgb = unchecked((int)0xaabbccdd),
        Tooltip = "description & <tag>\nnext", Modable = false, RadialDisplay = false,
        StartSoundData = "none", WarningSoundData = "none", ActiveInList = true
    };
    private static void RoundTripAndRuntime(string directory)
    {
        var path = Path.Combine(directory,"SpellTimers.xml");
        var definition = Definition();
        TimerDefinition saved;
        using (var host = new HostEnvironment(directory))
        {
            var timers = ActGlobals.oFormSpellTimers!;
            var root = ActGlobals.oFormActMain!;
            var revision = timers.ReadTimerDefinitions(0,16).Revision;
            revision = timers.SaveTimerDefinition(path,revision,null,definition);
            var item = timers.ReadTimerDefinitions(0,16).Items.Single();
            Check(item.Definition == definition,"Definition fields changed before round trip");
            Check(File.ReadAllText(path).Contains("<SpellTimers") && File.ReadAllText(path).Contains("Checked=\"true\""),"ACT XML element/attribute names missing");
            root.CurrentZone = definition.Category; root.LastKnownTime = DateTime.UnixEpoch;
            timers.NotifySpell("Source",definition.Name,true,"Victim",true);
            var frame = timers.GetTimerFrames().Single();
            var countdown = frame.SpellTimers.Single();
            var original = frame.TimerData;
            var started = countdown.StartTime;
            revision = timers.SaveTimerDefinition(path,revision,item.Key,definition with { TimerValue = 40, ActiveInList = false });
            Check(ReferenceEquals(frame.TimerData,original) && countdown.TimerFinalDuration == 20 && countdown.StartTime == started,
                "Editing reset/replaced a running frame or countdown");
            root.LastKnownTime = DateTime.UnixEpoch.AddSeconds(1);
            timers.NotifySpell("Source",definition.Name,true,"Victim",true);
            Check(ReferenceEquals(frame.TimerData,original) && frame.SpellTimers.Count == 1,"Disabled definition affected existing frame");
            saved = definition with { TimerValue = 40 };
            revision = timers.SaveTimerDefinition(path,revision,item.Key,saved);
            timers.NotifySpell("Source",definition.Name,true,"Victim",true);
            Check(!ReferenceEquals(frame.TimerData,original) && frame.TimerData.TimerValue == 40 && frame.SpellTimers.Count == 1 && countdown.TimerFinalDuration == 20,
                "Notify did not rebind before the two-second gate or altered captured duration");
            var liveDefinition = frame.TimerData;
            liveDefinition.Tooltip = "external"; liveDefinition.WarningValue = 7;
            revision = timers.ReadTimerDefinitions(0,16).Revision;
            // Genuine ACT writes boolean attributes using Boolean.ToString (True/False).
            File.WriteAllText(path,File.ReadAllText(path).Replace("=\"true\"","=\"True\"").Replace("=\"false\"","=\"False\""));
            revision = timers.ReloadTimerDefinitions(path,revision);
            Check(ReferenceEquals(frame.TimerData,liveDefinition) && liveDefinition.Tooltip == saved.Tooltip && liveDefinition.WarningValue == saved.WarningValue,
                "Reload did not update the existing registry object");
            Check(ReferenceEquals(frame.SpellTimers.Single(),countdown) && timers.ReadTimerDefinitions(0,16).RetainedFrames == 1,
                "Reload changed transient countdowns");
            var renamed = saved with { Name = "Renamed" };
            revision = timers.SaveTimerDefinition(path,revision,item.Key,renamed);
            Check(frame.Name == saved.Name && ReferenceEquals(frame.TimerData,liveDefinition),"Rename changed detached active frame");
            var key = timers.ReadTimerDefinitions(0,16).Items.Single().Key;
            revision = timers.DeleteTimerDefinition(path,revision,key);
            Check(timers.ReadTimerDefinitions(0,16).Total == 0 && timers.GetTimerFrames().Count == 1,"Delete canceled active timer");
            revision = timers.SaveTimerDefinition(path,revision,null,saved);
            Check(timers.ReadTimerDefinitions(0,16).Revision == revision,"Revision was not acknowledged");
        }
        using (var host = new HostEnvironment(directory))
        {
            var timers = ActGlobals.oFormSpellTimers!;
            var page = timers.ReadTimerDefinitions(0,16);
            Check(page.Items.Single().Definition == saved && page.RetainedFrames == 0,"Restart lost definition fields or restored live countdowns");
            // Same registry comparer applies to edit and delete keys, including canonically equivalent spell names.
            var alias = page.Items[0].Key.Normalize(NormalizationForm.FormD);
            var revision = timers.SaveTimerDefinition(path,page.Revision,alias,page.Items[0].Definition);
            revision = timers.DeleteTimerDefinition(path,revision,alias);
            Check(timers.ReadTimerDefinitions(0,16).Revision == revision,"Delete revision missing");
            Check(!File.ReadAllText(path).Contains("<Spell "),"Deleted timer remained on disk before normal shutdown repair");
        }
        using (var host = new HostEnvironment(directory))
            Check(ActGlobals.oFormSpellTimers!.ReadTimerDefinitions(0,16).Total == 0,"Empty definition file was not persisted");
    }
    private static void Failures(string directory)
    {
        var path = Path.Combine(directory,"SpellTimers.xml");
        using var host = new HostEnvironment(directory);
        var timers = ActGlobals.oFormSpellTimers!;
        var definition = Definition();
        var revision = timers.SaveTimerDefinition(path,timers.ReadTimerDefinitions(0,16).Revision,null,definition);
        var item = timers.ReadTimerDefinitions(0,16).Items.Single();
        var bytes = File.ReadAllBytes(path);
        foreach (var invalid in new[] { definition with { Name = "" }, definition with { TimerValue = -1 }, definition with { TimerValue = 3601 },
            definition with { WarningValue = 101 }, definition with { RemoveValue = 1 }, definition with { Tooltip = "bad\0text" },
            definition with { Tooltip = "invalid XML \u0001" } })
        {
            Reject(() => timers.SaveTimerDefinition(path,revision,item.Key,invalid));
            Check(File.ReadAllBytes(path).SequenceEqual(bytes) && timers.ReadTimerDefinitions(0,16).Items.Single() == item,"Invalid edit published or overwrote file");
        }
        Reject(() => timers.SaveTimerDefinition(path,revision,null,definition with { Name = definition.Name.ToUpper() }));
        revision = timers.SaveTimerDefinition(path,revision,null,definition with { Name = "Other" });
        Reject(() => timers.SaveTimerDefinition(path,revision,item.Key,definition with { Name = "Other" }));
        Reject(() => timers.SaveTimerDefinition(path,revision-1,item.Key,definition));
        var existing = timers.TimerDefs[item.Key]; existing.Tooltip = "plugin edit";
        Reject(() => timers.SaveTimerDefinition(path,revision,item.Key,definition));
        revision = timers.ReadTimerDefinitions(0,16).Revision;
        bytes = File.ReadAllBytes(path);
        var snapshot = timers.ReadTimerDefinitions(0,16).Items;
        File.Move(path,path+".saved"); Directory.CreateDirectory(path);
        try
        {
            Reject(() => timers.SaveTimerDefinition(path,revision,item.Key,definition with { Name = "Not published" }));
            Check(timers.ReadTimerDefinitions(0,16).Items.SequenceEqual(snapshot),"Disk failure changed registry");
            Check(File.ReadAllBytes(path+".saved").SequenceEqual(bytes) && Directory.GetFiles(directory,"*.tmp").Length == 0,"Disk failure damaged prior file or leaked temporary file");
        }
        finally { Directory.Delete(path); File.Move(path+".saved",path); }
        var firstName = System.Security.SecurityElement.Escape(definition.Name);
        var normalizedName = System.Security.SecurityElement.Escape(definition.Name.Normalize(NormalizationForm.FormD));
        var canonicalCollision = $"<SpellTimers><Spell Name='{firstName}' Category='Boss Zone' Tooltip='must not publish'/><Spell Name='{normalizedName}' Category='Boss Zone'/></SpellTimers>";
        foreach (var xml in new[] { canonicalCollision, "<SpellTimers><Spell>", "<SpellTimers><Spell Name='Same'/><Spell Name='SAME'/></SpellTimers>",
            "<!DOCTYPE x [<!ENTITY test 'bad'>]><SpellTimers/>", "<SpellTimers><Spell Name='bad' Checked='invalid'/></SpellTimers>", "<SpellTimers><Spell Name='bad' Checked='1'/></SpellTimers>" })
        {
            File.WriteAllText(path,xml);
            try
            {
                Reject(() => timers.ReloadTimerDefinitions(path,revision));
                Check(timers.ReadTimerDefinitions(0,16).Items.SequenceEqual(snapshot),"Corrupt reload partially published");
            }
            finally { File.WriteAllBytes(path,bytes); }
        }
        Reject(() => timers.ReadTimerDefinitions(-1,16));
        Reject(() => timers.ReadTimerDefinitions(0,17));
    }
    private static void LegacyValues(string directory)
    {
        var path = Path.Combine(directory,"SpellTimers.xml");
        File.WriteAllText(path,"<SpellTimers><Spell Name='raw' Category='plugin' Timer='-42' WarningValue='1234' RemoveValue='7' Checked='false'/></SpellTimers>");
        for (var index = 0; index < 2; index++)
        {
            using var host = new HostEnvironment(directory);
            var definition = ActGlobals.oFormSpellTimers!.ReadTimerDefinitions(0,16).Items.Single().Definition;
            Check(definition.TimerValue == -42 && definition.WarningValue == 1234 && definition.RemoveValue == 7 && !definition.ActiveInList,
                "Persistence clamped raw ACT/plugin values to editor limits");
            Check(definition.Panel1Display && definition.Modable && definition.RadialDisplay,"Missing attributes did not preserve ACT defaults");
        }
    }
    private static void StartupFailure(string directory)
    {
        var path = Path.Combine(directory,"SpellTimers.xml");
        const string invalid = "<SpellTimers><Spell Name='bad' Timer='invalid'/></SpellTimers>";
        File.WriteAllText(path,invalid);
        Reject(() => { using var host = new HostEnvironment(directory); });
        Check(ActGlobals.oFormActMain is null && ActGlobals.oFormSpellTimers is null && File.ReadAllText(path) == invalid,
            "Failed startup retained globals or rewrote the invalid file");
        File.WriteAllText(path,"<SpellTimers/>");
        using var recovered = new HostEnvironment(directory);
        Check(ActGlobals.oFormSpellTimers!.ReadTimerDefinitions(0,16).Total == 0,"Corrected settings could not start a fresh host");
    }

}
