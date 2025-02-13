try {
    delete Object.prototype
    console.log('a')
} catch (error) {
    console.log('b')
}
var index = 10
module.exports = index;