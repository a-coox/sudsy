class C {
    public static IResult Json<TValue>(
        TValue? data,
        int? statusCode = null
    )
#pragma warning restore RS0026 // Do not add overloads
        => TypedResults.Json(data, statusCode);

    public int Q() // q
        => 1;

    public int P
        => 1; // one
}
