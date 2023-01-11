{ mkDerivation, monodis, exec }:
mkDerivation
{
  builder= "monodis –output=$out ${exec}";
}
