export class MyTool {
    static formatSize = (size: number): string => {
        if (size === 0) {
            return '0 B';
        }
        const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB'];
        const i = Math.min(Math.floor(Math.log(size) / Math.log(1024)), units.length - 1);

        return `${(size / Math.pow(1024, i)).toFixed(2)} ${units[i]}`;
    }
}
