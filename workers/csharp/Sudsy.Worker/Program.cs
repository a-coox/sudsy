using Sudsy.Worker;

// Deep expression nesting can recurse far inside Roslyn; run on a thread with a large stack.
var exitCode = 0;
var thread = new Thread(
    () =>
    {
        using var stdin = Console.OpenStandardInput();
        using var stdout = Console.OpenStandardOutput();
        exitCode = new WorkerServer(stdin, stdout).Run();
    },
    maxStackSize: 256 * 1024 * 1024);
thread.Start();
thread.Join();
return exitCode;
