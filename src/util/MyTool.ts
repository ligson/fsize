export class MyTool {
    static formatSize = (size: number): string => {
        if (size === 0) {
            return '0 B';
        }
        const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB'];
        let i = Math.floor(Math.log(size) / Math.log(1024));

        // 调整 i 的值以满足你的显示要求
        if (i >= 3) { // 如果 size 大于等于 GB 的大小，则显示为 MB
            i = 2;
        } else if (i >= 2) { // 如果 size 大于等于 MB 的大小但小于 GB，则显示为 MB
            i = 2;
        } else if (i >= 1) { // 如果 size 大于等于 KB 的大小但小于 MB，则显示为 KB
            i = 1;
        }

        return `${(size / Math.pow(1024, i)).toFixed(2)} ${units[i]}`;
    }
}
