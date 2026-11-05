using System;
/// <summary>Text.</summary>
public class Periods
{
    /// <summary>No period</summary>
//                        ^ doc-text-ends-with-period
    public void NoPeriod() { }
    /// <summary>Has period.</summary>
    public void HasPeriod() { }
    /// <summary>Question?</summary>
//                        ^ doc-text-ends-with-period
    public void Question() { }
    /// <summary>Exclamation!</summary>
//                           ^ doc-text-ends-with-period
    public void Exclamation() { }
    /// <summary>Paren (like this.)</summary>
    public void ParenInside() { }
    /// <summary>Paren (like this).</summary>
    public void ParenOutside() { }
    /// <summary>Paren (like this)</summary>
//                                ^ doc-text-ends-with-period
    public void ParenNone() { }
    /// <summary>Quote "like this."</summary>
//                                 ^ doc-text-ends-with-period
    public void Quote() { }
    /// <summary>Quote "like this".</summary>
    public void QuoteAfter() { }
    /// <summary>Trailing comma,</summary>
//                             ^ doc-text-ends-with-period
    public void Comma() { }
    /// <summary>Trailing semicolon;</summary>
//                                 ^ doc-text-ends-with-period
    public void Semicolon() { }
    /// <summary>Paren (comma,)</summary>
//                           ^ doc-text-ends-with-period
    public void ParenComma() { }
    /// <summary>Paren (semicolon;)</summary>
//                               ^ doc-text-ends-with-period
    public void ParenSemicolon() { }
    /// <summary>Trailing colon:</summary>
//                              ^ doc-text-ends-with-period
    public void Colon() { }
    /// <summary>Text -or-</summary>
    public void Or() { }
    /// <summary>See <see cref="Periods"/></summary>
//                                        ^ doc-text-ends-with-period
    public void EndsWithSee() { }
    /// <summary>See <see cref="Periods"/>.</summary>
    public void SeeThenPeriod() { }
    /// <summary>See <see langword="null"/></summary>
//                                         ^ doc-text-ends-with-period
    public void EndsWithLangword() { }
    /// <summary>Code <c>x</c></summary>
//                            ^ doc-text-ends-with-period
    public void EndsWithCode() { }
    /// <summary>Code <c>x</c>.</summary>
    public void CodePeriod() { }
    /// <summary>Param <paramref name="a"/></summary>
//                                         ^ doc-text-ends-with-period
    public void EndsWithParamref(int a) { }
    /// <summary>Type <typeparamref name="T"/></summary>
//                                            ^ doc-text-ends-with-period
    public void EndsWithTypeparamref<T>() { }
    /// <summary><see cref="Periods"/></summary>
//                                    ^ doc-text-ends-with-period
    public void OnlySee() { }
    /// <summary>
    /// Several lines
    /// without a period
//                      ^ doc-text-ends-with-period
    /// </summary>
    public void MultiLine() { }
}
