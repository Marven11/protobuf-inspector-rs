# 任务

你需要在当前文件夹使用rust复刻protobuf-inspector的功能

完成一个功能相同的protobuf的解析器，使其可以解析`my-blob`文件输出和protobuf-inspector `python main.py < my-blob`相同的输出

但是protobuf-inspector的代码写得很烂，工程结构也不同，改进它的工程结构，把函数放在正确的位置！

# 要求

实现一个parse_main函数，输入是`&[u8]`, 输出是Result<_, Error>, 错误可以为EOF

解析数据的函数和格式化数据的函数分开

使用terminal工具运行并测试rust版的输出和python版的是否一致

显示bytes时不一定需要和protobuf-inspector相同：可以直接使用rust的Debug显示bytes

# 资源

./protobuf-inspector存放了protobuf-inspector的源码

