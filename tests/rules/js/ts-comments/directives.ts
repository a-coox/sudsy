let a = 1;
// @ts-ignore
^^^^^^^^^^^^^ ban-ts-comment
a = "";
// @ts-expect-error
^^^^^^^^^^^^^^^^^^^ ban-ts-comment
a = "";
// @ts-expect-error: not a number
a = "";
/* @ts-ignore */
^^^^^^^^^^^^^^^^ ban-ts-comment
/**
 * @ts-expect-error
 */
// @ts-check
// @ts-nocheck
/// @ts-ignore
^^^^^^^^^^^^^^ ban-ts-comment
