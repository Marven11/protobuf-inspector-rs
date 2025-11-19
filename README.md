# protobuf-inspector-rs

一个简单的学习项目，使用rust解析protobuf数据

## 使用示例

### 示例1：基本数据解析 (payload_1.bin)
**解析结果：**
```
root:
   1 <chunk> = "人类有三大欲望：饮食、繁殖、睡眠"
   2 <varint> = 114
   3 <chunk> = "李田所"
```

### 示例2：嵌套消息解析 (payload_2.bin)
**解析结果：**
```
root:
   1 <chunk> = message:
       10 <startgroup> = group (end 10)
       10 <32bit> = 0x53454343 / 1397048131 / +847237000000.0
       10 <startgroup> = group (end 10)
   2 <chunk> = "消息'人类有三大欲望：饮食、繁殖、睡眠'已接收"
   3 <varint> = 1763000501
```

### 示例3：简单用户数据 (payload_3.bin)
**解析结果：**
```
root:
   1 <chunk> = "user_114514"
   2 <varint> = 1
```

### 示例4：复杂用户数据 (payload_4.bin)
**解析结果：**
```
root:
   1 <chunk> = "user_114514"
   2 <chunk> = "李田所"
   3 <varint> = 24
   4 <chunk> = "tiansuo@example.com"
   5 <chunk> = "活跃用户"
   5 <chunk> = "VIP"
   5 <chunk> = "详细信息"
   5 <chunk> = "扩展数据"
```