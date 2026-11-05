using System;
/// <summary>Text.</summary>
public class Periods
{
    /// <summary>No period.</summary>
    public void NoPeriod() { }
    /// <summary>Has period.</summary>
    public void HasPeriod() { }
    /// <summary>Question?</summary>
    public void Question() { }
    /// <summary>Exclamation!</summary>
    public void Exclamation() { }
    /// <summary>Paren (like this.)</summary>
    public void ParenInside() { }
    /// <summary>Paren (like this).</summary>
    public void ParenOutside() { }
    /// <summary>Paren (like this).</summary>
    public void ParenNone() { }
    /// <summary>Quote "like this."</summary>
    public void Quote() { }
    /// <summary>Quote "like this".</summary>
    public void QuoteAfter() { }
    /// <summary>Trailing comma.</summary>
    public void Comma() { }
    /// <summary>Trailing semicolon.</summary>
    public void Semicolon() { }
    /// <summary>Paren (comma.)</summary>
    public void ParenComma() { }
    /// <summary>Paren (semicolon.)</summary>
    public void ParenSemicolon() { }
    /// <summary>Trailing colon:</summary>
    public void Colon() { }
    /// <summary>Text -or-</summary>
    public void Or() { }
    /// <summary>See <see cref="Periods"/>.</summary>
    public void EndsWithSee() { }
    /// <summary>See <see cref="Periods"/>.</summary>
    public void SeeThenPeriod() { }
    /// <summary>See <see langword="null"/>.</summary>
    public void EndsWithLangword() { }
    /// <summary>Code <c>x</c>.</summary>
    public void EndsWithCode() { }
    /// <summary>Code <c>x</c>.</summary>
    public void CodePeriod() { }
    /// <summary>Param <paramref name="a"/>.</summary>
    public void EndsWithParamref(int a) { }
    /// <summary>Type <typeparamref name="T"/>.</summary>
    public void EndsWithTypeparamref<T>() { }
    /// <summary><see cref="Periods"/>.</summary>
    public void OnlySee() { }
    /// <summary>
    /// Several lines
    /// without a period.
    /// </summary>
    public void MultiLine() { }
}
