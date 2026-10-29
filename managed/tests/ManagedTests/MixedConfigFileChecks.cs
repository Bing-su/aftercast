using Aftercast.ActCompat;
using System.Diagnostics;
using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Text;

internal static class MixedConfigFileChecks
{
    private static readonly byte[] OldTrigger=Encoding.UTF8.GetBytes("old trigger"),OldTimer=Encoding.UTF8.GetBytes("old timer"),NewTrigger=Encoding.UTF8.GetBytes("new trigger"),NewTimer=Encoding.UTF8.GetBytes("new timer");
    private static void Check(bool value,string message) { if(!value)throw new Exception(message); }
    private static void Seed(string directory) { File.WriteAllBytes(Path.Combine(directory,"CustomTriggers.xml"),OldTrigger);File.WriteAllBytes(Path.Combine(directory,"SpellTimers.xml"),OldTimer); }
    private static void Old(string directory) { Check(File.ReadAllBytes(Path.Combine(directory,"CustomTriggers.xml")).SequenceEqual(OldTrigger),"trigger rollback");Check(File.ReadAllBytes(Path.Combine(directory,"SpellTimers.xml")).SequenceEqual(OldTimer),"timer rollback"); }
    private static void Commit(string directory,Action<string> fault)
    {
        try { typeof(MixedConfigFiles).GetMethod("CommitCore",BindingFlags.NonPublic|BindingFlags.Static)!.Invoke(null,[directory,OldTrigger,OldTimer,NewTrigger,NewTimer,fault]); }
        catch(TargetInvocationException error) when(error.InnerException is not null) { ExceptionDispatchInfo.Capture(error.InnerException).Throw(); }
    }
    internal static void Child(string directory,string stage) => Commit(directory,at=> { if(at==stage) Environment.Exit(23); });
    internal static void Run()
    {
        var directory=Directory.CreateTempSubdirectory("aftercast-mixed-files-");
        try
        {
            foreach(var stage in new[]{"before-journal","journal","CustomTriggers.xml","SpellTimers.xml","before-commit"})
            {
                Seed(directory.FullName);
                try { Commit(directory.FullName,at=>{if(at==stage)throw new IOException("synthetic stage failure");});throw new Exception("fault not reached"); }
                catch(IOException) { Old(directory.FullName); }
            }
            foreach(var stage in new[]{"journal","CustomTriggers.xml","SpellTimers.xml","before-commit"})
            {
                Seed(directory.FullName);
                var start=new ProcessStartInfo(Environment.ProcessPath!){UseShellExecute=false};
                if(Path.GetFileNameWithoutExtension(Environment.ProcessPath)=="dotnet") start.ArgumentList.Add(typeof(Program).Assembly.Location);
                start.ArgumentList.Add("mixed-config-crash");start.ArgumentList.Add(directory.FullName);start.ArgumentList.Add(stage);
                using var child=Process.Start(start)!;
                if(!child.WaitForExit(10000)){child.Kill(true);throw new Exception("synthetic child timeout");}
                Check(child.ExitCode==23,"child did not exit at publication boundary");
                MixedConfigFiles.Recover(directory.FullName); Old(directory.FullName);
                MixedConfigFiles.Recover(directory.FullName); Old(directory.FullName);
            }
            Seed(directory.FullName);
            try { Commit(directory.FullName,at=>{if(at=="CustomTriggers.xml"){File.WriteAllText(Path.Combine(directory.FullName,"SpellTimers.xml"),"external");throw new IOException("conflict");}});throw new Exception("conflict accepted"); }
            catch(AggregateException) { Check(File.ReadAllText(Path.Combine(directory.FullName,"SpellTimers.xml"))=="external","external edit overwritten"); }
            try { MixedConfigFiles.Recover(directory.FullName);throw new Exception("recovery conflict accepted"); } catch(IOException) { }
            File.WriteAllBytes(Path.Combine(directory.FullName,"SpellTimers.xml"),OldTimer);MixedConfigFiles.Recover(directory.FullName);Old(directory.FullName);
            using(var lease=new FileStream(Path.Combine(directory.FullName,".mixed-config.lock"),FileMode.Open,FileAccess.ReadWrite,FileShare.None))
                try { MixedConfigFiles.Commit(directory.FullName,OldTrigger,OldTimer,NewTrigger,NewTimer);throw new Exception("concurrent lease accepted"); } catch(IOException) { }
            MixedConfigFiles.Commit(directory.FullName,OldTrigger,OldTimer,NewTrigger,NewTimer);
            try { MixedConfigFiles.Commit(directory.FullName,OldTrigger,OldTimer,NewTrigger,NewTimer);throw new Exception("stale preview accepted"); } catch(IOException) { }
            Check(File.ReadAllBytes(Path.Combine(directory.FullName,"CustomTriggers.xml")).SequenceEqual(NewTrigger),"success trigger");
            Check(File.ReadAllBytes(Path.Combine(directory.FullName,"SpellTimers.xml")).SequenceEqual(NewTimer),"success timer");
            Console.WriteLine("Mixed Config file core: stage faults, child exits/recovery, external conflict, lease and stale bytes passed.");
        }
        finally { directory.Delete(true); }
    }
}
