using System.Text.Json;

namespace Aftercast.ActCompat;

// A recoverable publication unit for the two existing definition files only.
// It is not an ACT plugin API or a general transaction manager.
public static class MixedConfigFiles
{
    private const int Limit = 4 * 1024 * 1024;
    private const string Journal = ".mixed-config.json";
    private static readonly string[] Names = ["CustomTriggers.xml", "SpellTimers.xml"];
    private sealed record Entry(byte[]? Before, byte[] After);
    private sealed record Record(int Version, Entry[] Entries);
    private static FileStream Lease(string directory) => new(Path.Combine(directory,".mixed-config.lock"),FileMode.OpenOrCreate,FileAccess.ReadWrite,FileShare.None);
    private static byte[] ReadBounded(string path,int limit)
    {
        using var source=new FileStream(path,FileMode.Open,FileAccess.Read,FileShare.Read);
        using var output=new MemoryStream(); var buffer=new byte[8192]; int count;
        while((count=source.Read(buffer))!=0)
        {
            if(output.Length+count>limit) throw new InvalidDataException("Mixed Config file exceeds limit");
            output.Write(buffer,0,count);
        }
        return output.ToArray();
    }
    private static byte[]? Read(string path) => File.Exists(path) ? ReadBounded(path,Limit) : null;
    private static bool Same(byte[]? left, byte[]? right) => left is null ? right is null : right is not null && left.AsSpan().SequenceEqual(right);
    private static void Write(string path, byte[] bytes)
    {
        var temporary=path+"."+Guid.NewGuid().ToString("N")+".tmp";
        try
        {
            using(var stream=new FileStream(temporary,FileMode.CreateNew,FileAccess.Write,FileShare.None)) { stream.Write(bytes); stream.Flush(true); }
            File.Move(temporary,path,true);
        }
        finally { if(File.Exists(temporary)) File.Delete(temporary); }
    }
    private static void CheckOwned(string directory, Record record)
    {
        for(var i=0;i<Names.Length;i++)
        {
            var current=Read(Path.Combine(directory,Names[i])); var item=record.Entries[i];
            if(!Same(current,item.Before) && !Same(current,item.After)) throw new IOException("Mixed Config recovery conflicts with an external edit: "+Names[i]);
        }
    }
    private static void Rollback(string directory, Record record)
    {
        CheckOwned(directory,record);
        for(var i=0;i<Names.Length;i++)
        {
            CheckOwned(directory,record);
            var path=Path.Combine(directory,Names[i]); var before=record.Entries[i].Before;
            if(before is null) File.Delete(path); else Write(path,before);
        }
        File.Delete(Path.Combine(directory,Journal));
    }
    private static void RecoverOwned(string directory)
    {
        var path=Path.Combine(directory,Journal);
        if(!File.Exists(path)) return;
        var record=JsonSerializer.Deserialize<Record>(ReadBounded(path,24*1024*1024)) ?? throw new InvalidDataException("Invalid mixed Config journal");
        if(record.Version!=1 || record.Entries is not {Length:2} || record.Entries.Any(item=>item is null || item.After is null || item.After.Length>Limit || item.Before?.Length>Limit))
            throw new InvalidDataException("Invalid mixed Config journal");
        Rollback(directory,record);
    }
    // Called while the owning registry gate is held. A pending journal is resolved
    // before any standalone writer can publish into the same directory.
    internal static IDisposable AcquireSingle(string path)
    {
        var directory=Path.GetDirectoryName(Path.GetFullPath(path)) ?? throw new ArgumentException("Missing definition directory");
        var lease=Lease(directory);
        try { RecoverOwned(directory); return lease; }
        catch { lease.Dispose(); throw; }
    }
    /// Restore an interrupted publication before either registry is loaded. Conflicts fail closed.
    public static void Recover(string directory)
    {
        // Ordinary startup creates no transaction files when recovery is unnecessary.
        if(!File.Exists(Path.Combine(directory,Journal))) return;
        using var lease=Lease(directory); RecoverOwned(directory);
    }
    /// Publish validated definition bytes. Callers must hold both registry gates,
    /// suspend parsing, and prebuild a nonthrowing memory publication before calling.
    public static void Commit(string directory, byte[]? expectedTriggers, byte[]? expectedTimers, byte[] triggers, byte[] timers) =>
        CommitCore(directory,expectedTriggers,expectedTimers,triggers,timers,null);
    private static void CommitCore(string directory, byte[]? expectedTriggers, byte[]? expectedTimers, byte[] triggers, byte[] timers, Action<string>? fault)
    {
        ArgumentNullException.ThrowIfNull(triggers); ArgumentNullException.ThrowIfNull(timers);
        if(triggers.Length>Limit || timers.Length>Limit || expectedTriggers?.Length>Limit || expectedTimers?.Length>Limit) throw new InvalidDataException("Mixed Config definition exceeds limit");
        var record=new Record(1,[new(expectedTriggers?.ToArray(),triggers.ToArray()),new(expectedTimers?.ToArray(),timers.ToArray())]);
        using var lease=Lease(directory);
        RecoverOwned(directory);
        for(var i=0;i<Names.Length;i++) if(!Same(Read(Path.Combine(directory,Names[i])),record.Entries[i].Before)) throw new IOException("Mixed Config files changed since preview");
        fault?.Invoke("before-journal");
        Write(Path.Combine(directory,Journal),JsonSerializer.SerializeToUtf8Bytes(record));
        try
        {
            fault?.Invoke("journal");
            for(var i=0;i<Names.Length;i++)
            {
                CheckOwned(directory,record);
                Write(Path.Combine(directory,Names[i]),record.Entries[i].After);
                fault?.Invoke(Names[i]);
            }
            CheckOwned(directory,record);
            fault?.Invoke("before-commit");
            File.Delete(Path.Combine(directory,Journal));
        }
        catch(Exception original)
        {
            try { Rollback(directory,record); }
            catch(Exception cleanup) { throw new AggregateException("Mixed Config publication and recovery failed; journal retained",original,cleanup); }
            throw;
        }
    }
}
