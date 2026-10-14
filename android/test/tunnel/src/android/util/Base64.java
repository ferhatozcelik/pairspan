package android.util;
public class Base64 {
 public static final int NO_WRAP=2, DEFAULT=0;
 public static String encodeToString(byte[] b,int o,int n,int f){return java.util.Base64.getEncoder().encodeToString(java.util.Arrays.copyOfRange(b,o,o+n));}
 public static byte[] decode(String s,int f){return java.util.Base64.getDecoder().decode(s);}
}
