using System;

namespace Fx.SyntaxRegions
{
    public class Regions
    {
        public int[] Values = new int[]
        {
            #region InInitializer
            1,
            #endregion
        };

        public int Prop
        {
            #region InAccessorList
            get { return 1; }
            #endregion
        }

        public int Arrow =>
            #region InArrow
            1;
            #endregion

        public void Method(int x)
        {
            #region InMethod
            int y = x;
            #endregion
            switch (x)
            {
                #region InSwitch
                case 1:
                    break;
                #endregion
            }
            Action a = () =>
            {
                #region InLambda
                y++;
                #endregion
            };
        }

        #region BetweenMembers
        public void Other()
        {
        }
        #endregion
    }
}
