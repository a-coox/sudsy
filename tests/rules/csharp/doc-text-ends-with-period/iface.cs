using System;
/// <summary>Interface</summary>
//                    ^ doc-text-ends-with-period
public interface IFace
{
    /// <summary>Member</summary>
//                     ^ doc-text-ends-with-period
    void Run();
}
/// <summary>Record</summary>
//                 ^ doc-text-ends-with-period
public record Record(int A);
