try {
    delete Object.prototype
    console.log('a')
} catch (error) {
    console.log('b')
}
var index = 10
module.exports = index;


// 这类问题 rollup  的方案是引入 @rollup/plugin-commonjs 插件
// 但是依旧存在很多边界 case
