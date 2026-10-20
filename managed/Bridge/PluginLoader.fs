namespace Bxivact.Bridge

open System
open System.Collections.Generic
open System.IO
open System.Reflection
open System.Runtime.Loader
open System.Security.Cryptography
open Advanced_Combat_Tracker

/// Share the bridge's contract even when netcorehost loads the bridge outside the default context.
type private PluginContext(path: string, hash: byte[]) =
    inherit AssemblyLoadContext("bxivact:" + path)
    let resolver = AssemblyDependencyResolver(path)
    override this.Load(name) =
        let shared =
            [ typeof<IActPluginV1>.Assembly; typeof<System.Windows.Forms.TabPage>.Assembly
              typeof<list<int>>.Assembly ]
            |> List.tryFind (fun assembly -> assembly.GetName().Name = name.Name)
        match shared with
        | Some assembly ->
            if name.Name = "Advanced Combat Tracker" && name.FullName <> assembly.GetName().FullName then
                invalidOp ("Unsupported ACT reference: " + name.FullName)
            assembly
        | None ->
            match name.Name with
            | "FFXIV_ACT_Plugin.Memory" when LinuxPluginCompatibility.isKnownPlugin hash ->
                let plugin = this.Assemblies |> Seq.find (fun assembly -> assembly.Location = path)
                LinuxPluginCompatibility.loadMemory this plugin
            // Share the preloaded packages with both file-loaded and Costura byte-loaded dependencies.
            | "System.Security.Permissions" | "System.Windows.Extensions" ->
                AssemblyLoadContext.Default.LoadFromAssemblyName(name)
            | _ ->
                let resolved = resolver.ResolveAssemblyToPath(name)
                let sibling = Path.Combine(Path.GetDirectoryName(path), name.Name + ".dll")
                if not (isNull resolved) then this.LoadFromAssemblyPath(resolved)
                elif File.Exists(sibling) then this.LoadFromAssemblyPath(sibling)
                // Let framework and Costura's embedded-assembly resolution run when no file exists.
                else null
    override this.LoadUnmanagedDll(name) =
        let resolved = resolver.ResolveUnmanagedDllToPath(name)
        if isNull resolved then IntPtr.Zero else this.LoadUnmanagedDllFromPath(resolved)

module internal PluginLoader =
    // Constructors may register global handlers (for example Costura); reuse assemblies across sessions.
    let private assemblies = Dictionary<string, byte[] * Assembly>(if OperatingSystem.IsWindows() then StringComparer.OrdinalIgnoreCase else StringComparer.Ordinal)

    let private load path =
        let hash = SHA256.HashData(File.ReadAllBytes(path))
        match assemblies.TryGetValue(path) with
        | true, (previous, assembly) ->
            if previous <> hash then invalidOp "Plugin file changed after loading; restart the process before replacing plugins"
            assembly
        | _ ->
            let context = PluginContext(path, hash)
            let assembly = context.LoadFromAssemblyPath(path)
            assemblies.Add(path, (hash, assembly))
            assembly

    let create path typeName =
        let mutable stage = "load"
        try
            let path = Path.GetFullPath(path)
            let assembly = load path
            stage <- "select type"
            let eligible (candidate: Type) =
                candidate.IsVisible && candidate.IsClass && not candidate.IsAbstract
                && not candidate.ContainsGenericParameters
                && typeof<IActPluginV1>.IsAssignableFrom(candidate)
                && not (isNull (candidate.GetConstructor(Type.EmptyTypes)))
            let candidate =
                match typeName with
                | Some name ->
                    let candidate = assembly.GetType(name, true, false)
                    if not (eligible candidate) then invalidOp "Type must be a public concrete IActPluginV1 with a public parameterless constructor"
                    candidate
                | None ->
                    match assembly.GetTypes() |> Array.filter eligible with
                    | [| candidate |] -> candidate
                    | candidates -> invalidOp (sprintf "Expected one constructible IActPluginV1, found %d; specify type_name" candidates.Length)
            stage <- "construct " + candidate.FullName
            Activator.CreateInstance(candidate)
        with error ->
            let details =
                match error with
                | :? ReflectionTypeLoadException as types ->
                    types.LoaderExceptions |> Array.map (fun failure -> failure.Message) |> String.concat "; "
                | _ -> error.GetBaseException().Message
            raise (InvalidOperationException(sprintf "Plugin %s failed for %s: %s" stage path details, error))
