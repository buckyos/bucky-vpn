import 'dart:async';
import 'package:dio/browser.dart';
import 'package:dio/dio.dart';

class HttpResult {
  final int code;
  final String? msg;

  HttpResult(this.code, {this.msg});

  // 添加 isSuccess 判断，code == 0 时为成功
  bool get isSuccess => code == 0;
}

class HttpClient {
  final String baseUrl;
  final dio = Dio();

  HttpClient(this.baseUrl) {
    dio.httpClientAdapter = BrowserHttpClientAdapter();
  }

  Future<void> get(String path) async {
    print('GET: $baseUrl$path');
  }

  get _baseUrl {
    if (baseUrl.endsWith("/")) {
      return baseUrl.substring(0, baseUrl.length - 1);
    } else {
      return "$baseUrl";
    }
  }

  Future<(HttpResult, dynamic?)> getJson(String path, {Map<String, dynamic>? body, Map<String, String>? headers, ProgressCallback? onReceiveProgress}) async {
    final url = "${_baseUrl}${path}";

    var tryCount = 0;
    while (tryCount < 3) {
      try {
        var response = await dio.get(url,
            queryParameters: body,
            onReceiveProgress: onReceiveProgress,
            options: Options(
                sendTimeout: const Duration(seconds: 20),
                receiveTimeout: const Duration(seconds: 20),
                headers: headers,
                contentType: Headers.formUrlEncodedContentType));
        if (response.statusCode == 200) {
          return (HttpResult(0), response.data);
        } else {
          return (HttpResult(response.statusCode ?? -1), null);
        }
      } on Exception catch (_) {
        tryCount++;
        continue;
      }
    }
    return (HttpResult(-1), null);
  }

  Future<(HttpResult, dynamic?)> postForm(String path, Map<String, dynamic>? body, Map<String, String>? headers, {ProgressCallback? onSendProgress, ProgressCallback? onReceiveProgress}) async {
    final url = "${_baseUrl}${path}";

    var tryCount = 0;
    while (tryCount < 3) {
      try {
        var response = await dio.post(url,
            data: body,
            onSendProgress: onSendProgress,
            onReceiveProgress: onReceiveProgress,
            options: Options(
                sendTimeout: const Duration(seconds: 20),
                receiveTimeout: const Duration(seconds: 20),
                headers: headers,
                contentType: Headers.formUrlEncodedContentType));
        if (response.statusCode == 200) {
          return (HttpResult(0), response.data);
        } else {
          return (HttpResult(response.statusCode ?? 401), null);
        }
      } on Exception catch (_) {
        tryCount++;
        continue;
      }
    }
    return (HttpResult(-1), null);
  }

  Future<(HttpResult, dynamic?)> postJson(String path, Map<String, dynamic>? body, {Map<String, String>? headers, ProgressCallback? onSendProgress, ProgressCallback? onReceiveProgress}) async {
    final url = "${_baseUrl}${path}";

    var tryCount = 0;
    while (tryCount < 3) {
      try {
        var response = await dio.post(url,
            data: body,
            onSendProgress: onSendProgress,
            onReceiveProgress: onReceiveProgress,
            options: Options(
                sendTimeout: const Duration(seconds: 20),
                receiveTimeout: const Duration(seconds: 20),
                headers: headers,
                contentType: Headers.jsonContentType));
        if (response.statusCode == 200) {
          return (HttpResult(0), response.data);
        } else {
          return (HttpResult(response.statusCode ?? 401), null);
        }
      } on Exception catch (_) {
        tryCount++;
        continue;
      }
    }
    return (HttpResult(-1), null);
  }

  Future<(HttpResult, dynamic?)> upload(String path, List<String> fileList, Map<String, String>? headers, void Function(int, int)? onProgress) async {
    final url = "${_baseUrl}${path}";

    var files = [];
    for (var file in fileList) {
      files.add(await MultipartFile.fromFile(file, filename: file.split("/").last));
    }
    final formData = FormData.fromMap({
      "name": 'file',
      "date": DateTime.now().toIso8601String(),
      "file": files,
    });

    try {
      final response = await dio.post(url,
          data: formData,
          onSendProgress: onProgress,
          options: Options(
              contentType: "multipart/form-data", headers: headers));
      if (response.statusCode == 200) {
        return (HttpResult(0), response.data);
      } else {
        return (HttpResult(response.statusCode ?? 401), null);
      }
    } on Exception catch (_) {
      return (HttpResult(-1), null);
    }
  }

  Future<HttpResult> download(String url, String savePath, Map<String, String>? headers, ProgressCallback? onProgress) async {
    try {
      final response = await dio.download(url, savePath, options: Options(
        headers: headers
      ), onReceiveProgress: onProgress);
      if (response.statusCode == 200) {
        return HttpResult(0);
      } else {
        return HttpResult(response.statusCode ?? 401);
      }
    } on Exception catch (_) {
      return HttpResult(-1);
    }
  }
}
