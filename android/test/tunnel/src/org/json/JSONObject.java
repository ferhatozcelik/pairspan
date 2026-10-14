package org.json;
public class JSONObject {
 public final java.util.Map<String,Object> values=new java.util.HashMap<>();
 public JSONObject put(String k,Object v){values.put(k,v);return this;}
}
