import React, { useState } from "react";
//              ^^^^^^^^ no-unused-vars
import type { FC } from "react";
export const A: FC = () => <div />;
type Mapped = { [K in "a"]: 1 };
//   ^^^^^^ no-unused-vars
namespace B.C {}
declare module "m" { interface Ambient {} var ambient: number; }
